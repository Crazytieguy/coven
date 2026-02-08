use std::process::Command as StdCommand;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyEvent};
use crossterm::terminal;
use futures::StreamExt;
use tokio::sync::mpsc;

use coven::display::input::{InputAction, InputHandler};
use coven::display::renderer::Renderer;
use coven::event::{AppEvent, InputMode};
use coven::protocol::types::InboundEvent;
use coven::session::runner::SessionRunner;
use coven::session::state::{SessionState, SessionStatus};

use super::handle_inbound;

/// How a session ended.
pub enum SessionOutcome {
    /// Session produced a result (normal completion).
    Completed { result_text: String },
    /// User pressed Ctrl+C.
    Interrupted,
    /// Claude process exited unexpectedly.
    ProcessExited,
}

/// Per-session transient state for event buffering and follow-ups.
struct SessionLocals {
    event_buffer: Vec<AppEvent>,
    pending_followup: Option<String>,
    result_text: String,
}

/// Run a single session's event loop with full input support.
///
/// Handles streaming display, event buffering during input, steering messages,
/// follow-up messages, message viewing, and interrupt/end-session signals.
///
/// Returns when the session produces a Result event, the user interrupts,
/// or the process exits.
pub async fn run_session(
    runner: &mut SessionRunner,
    state: &mut SessionState,
    renderer: &mut Renderer,
    input: &mut InputHandler,
    event_rx: &mut mpsc::UnboundedReceiver<AppEvent>,
    term_events: &mut EventStream,
) -> Result<SessionOutcome> {
    let mut locals = SessionLocals {
        event_buffer: Vec::new(),
        pending_followup: None,
        result_text: String::new(),
    };

    loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Some(app_event) => {
                        if input.is_active() && state.status == SessionStatus::Running {
                            locals.event_buffer.push(app_event);
                        } else {
                            let outcome = process_claude_event(
                                app_event, state, renderer, runner, &mut locals,
                            ).await?;
                            if let Some(outcome) = outcome {
                                return Ok(outcome);
                            }
                        }
                    }
                    None => return Ok(SessionOutcome::ProcessExited),
                }
            }

            term_event = term_events.next() => {
                match term_event {
                    Some(Ok(Event::Key(key_event))) => {
                        let action = handle_session_key_event(
                            &key_event, input, renderer, runner, state, &mut locals,
                        ).await?;
                        match action {
                            LoopAction::Continue => {}
                            LoopAction::Interrupted => return Ok(SessionOutcome::Interrupted),
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => return Ok(SessionOutcome::ProcessExited),
                }
            }
        }
    }
}

/// Flow control signals from key event handlers.
enum LoopAction {
    Continue,
    Interrupted,
}

/// Handle a key event during an active session.
async fn handle_session_key_event(
    key_event: &KeyEvent,
    input: &mut InputHandler,
    renderer: &mut Renderer,
    runner: &mut SessionRunner,
    state: &mut SessionState,
    locals: &mut SessionLocals,
) -> Result<LoopAction> {
    let action = input.handle_key(key_event);
    match action {
        InputAction::Activated(c) => {
            renderer.begin_input_line();
            renderer.write_raw(&c.to_string());
        }
        InputAction::Submit(text, mode) => {
            flush_event_buffer(locals, state, renderer);
            match mode {
                InputMode::Steering => {
                    renderer.render_user_message(&text);
                    runner.send_message(&text).await?;
                }
                InputMode::FollowUp => {
                    if state.status == SessionStatus::WaitingForInput {
                        renderer.render_user_message(&text);
                        runner.send_message(&text).await?;
                        state.status = SessionStatus::Running;
                    } else {
                        // Don't render yet — will render when dispatched after Result
                        locals.pending_followup = Some(text);
                    }
                }
            }
        }
        InputAction::ViewMessage(n) => {
            view_message(renderer, n);
            flush_event_buffer(locals, state, renderer);
            if state.status == SessionStatus::WaitingForInput {
                renderer.show_prompt();
                input.activate();
            }
        }
        InputAction::Interrupt => {
            runner.kill().await?;
            return Ok(LoopAction::Interrupted);
        }
        InputAction::EndSession => {
            runner.close_input();
        }
        InputAction::Cancel => {
            flush_event_buffer(locals, state, renderer);
            if state.status == SessionStatus::WaitingForInput {
                renderer.show_prompt();
                input.activate();
            }
        }
        InputAction::None => {}
    }
    Ok(LoopAction::Continue)
}

/// Process a single claude event. Returns Some(outcome) if the session should end.
async fn process_claude_event(
    event: AppEvent,
    state: &mut SessionState,
    renderer: &mut Renderer,
    runner: &mut SessionRunner,
    locals: &mut SessionLocals,
) -> Result<Option<SessionOutcome>> {
    match event {
        AppEvent::Claude(inbound) => {
            handle_inbound(&inbound, state, renderer);

            // Capture result text
            if let InboundEvent::Result(ref result) = *inbound {
                locals.result_text.clone_from(&result.result);
            }

            // If result and there's a pending follow-up, send it
            if matches!(*inbound, InboundEvent::Result(_)) {
                if let Some(text) = locals.pending_followup.take() {
                    renderer.render_user_message(&text);
                    runner.send_message(&text).await?;
                    state.status = SessionStatus::Running;
                } else {
                    return Ok(Some(SessionOutcome::Completed {
                        result_text: locals.result_text.clone(),
                    }));
                }
            }
        }
        AppEvent::ParseWarning(warning) => {
            renderer.render_warning(&warning);
        }
        AppEvent::ProcessExit(code) => {
            renderer.render_exit(code);
            state.status = SessionStatus::Ended;
            return Ok(Some(SessionOutcome::ProcessExited));
        }
    }
    Ok(None)
}

/// Flush all buffered events through the renderer.
fn flush_event_buffer(
    locals: &mut SessionLocals,
    state: &mut SessionState,
    renderer: &mut Renderer,
) {
    for event in locals.event_buffer.drain(..) {
        match event {
            AppEvent::Claude(inbound) => {
                // Capture result text from buffered events too
                if let InboundEvent::Result(ref result) = *inbound {
                    locals.result_text.clone_from(&result.result);
                }
                handle_inbound(&inbound, state, renderer);
            }
            AppEvent::ParseWarning(warning) => {
                renderer.render_warning(&warning);
            }
            AppEvent::ProcessExit(code) => {
                renderer.render_exit(code);
                state.status = SessionStatus::Ended;
            }
        }
    }
}

/// What the user chose to do after a session completed.
pub enum FollowUpAction {
    /// User sent a follow-up message; continue the session.
    Sent,
    /// User wants to end the session (Ctrl+D, Ctrl+C, etc.).
    Exit,
}

/// Show a prompt and wait for user to type a follow-up or exit.
pub async fn wait_for_followup(
    input: &mut InputHandler,
    renderer: &mut Renderer,
    runner: &mut SessionRunner,
    state: &mut SessionState,
    term_events: &mut EventStream,
) -> Result<FollowUpAction> {
    match wait_for_text_input(input, renderer, term_events).await? {
        Some(text) => {
            runner.send_message(&text).await?;
            state.status = SessionStatus::Running;
            Ok(FollowUpAction::Sent)
        }
        None => Ok(FollowUpAction::Exit),
    }
}

/// Show a prompt and wait for user input. Returns the text, or None to exit.
///
/// Unlike `wait_for_followup`, this doesn't send the message to a runner —
/// the caller decides what to do with the text (e.g. spawn a resumed session).
pub async fn wait_for_user_input(
    input: &mut InputHandler,
    renderer: &mut Renderer,
    term_events: &mut EventStream,
) -> Result<Option<String>> {
    wait_for_text_input(input, renderer, term_events).await
}

/// Wait for user to type and submit text, or exit.
///
/// Shows the prompt, activates input, and loops on terminal events.
/// Returns the submitted text, or None if the user interrupted/ended.
async fn wait_for_text_input(
    input: &mut InputHandler,
    renderer: &mut Renderer,
    term_events: &mut EventStream,
) -> Result<Option<String>> {
    renderer.show_prompt();
    input.activate();

    loop {
        match term_events.next().await {
            Some(Ok(Event::Key(key_event))) => {
                let action = input.handle_key(&key_event);
                match action {
                    InputAction::Submit(text, _) => {
                        renderer.render_user_message(&text);
                        return Ok(Some(text));
                    }
                    InputAction::ViewMessage(n) => {
                        view_message(renderer, n);
                        renderer.show_prompt();
                        input.activate();
                    }
                    InputAction::Cancel => {
                        renderer.show_prompt();
                        input.activate();
                    }
                    InputAction::Interrupt | InputAction::EndSession => {
                        return Ok(None);
                    }
                    InputAction::Activated(_) | InputAction::None => {}
                }
            }
            Some(Ok(_)) => {}
            Some(Err(_)) | None => return Ok(None),
        }
    }
}

/// Open message N in $PAGER.
pub fn view_message(renderer: &mut Renderer, n: usize) {
    let messages = renderer.messages();
    if n == 0 || n > messages.len() {
        renderer.write_raw(&format!("No message {n}\r\n"));
        return;
    }

    let msg = &messages[n - 1];
    let content = match &msg.result {
        Some(result) => format!(
            "{}\n\n{}\n\n--- Result ---\n\n{}",
            msg.label, msg.content, result
        ),
        None => format!("{}\n\n{}", msg.label, msg.content),
    };

    // Leave raw mode so the pager can handle keyboard input.
    // The pager manages its own alternate screen.
    terminal::disable_raw_mode().ok();

    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".to_string());
    let mut child = StdCommand::new(&pager)
        .arg("-R") // handle ANSI colors
        .stdin(std::process::Stdio::piped())
        .spawn();

    if let Ok(ref mut child) = child {
        if let Some(ref mut stdin) = child.stdin {
            use std::io::Write;
            stdin.write_all(content.as_bytes()).ok();
        }
        // Close stdin so pager reads EOF
        child.stdin.take();
        child.wait().ok();
    }

    terminal::enable_raw_mode().ok();
}
