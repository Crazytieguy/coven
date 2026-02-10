use std::io::Write;
use std::process::Command as StdCommand;

use anyhow::Result;
use crossterm::event::{Event, KeyEvent};
use crossterm::terminal;

use crate::display::input::{InputAction, InputHandler};
use crate::display::renderer::Renderer;
use crate::event::{AppEvent, InputMode};
use crate::fork::{self, ForkConfig};
use crate::handle_inbound;
use crate::protocol::types::InboundEvent;
use crate::session::runner::{SessionConfig, SessionRunner};
use crate::session::state::{SessionState, SessionStatus};
use crate::vcr::{Io, IoEvent, VcrContext};

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
    pending_followups: Vec<String>,
    result_text: String,
}

/// Run a single session's event loop with full input support.
///
/// Handles streaming display, event buffering during input, steering messages,
/// follow-up messages, message viewing, and interrupt/end-session signals.
///
/// Returns when the session produces a Result event, the user interrupts,
/// or the process exits.
pub async fn run_session<W: Write>(
    runner: &mut SessionRunner,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
    input: &mut InputHandler,
    io: &mut Io,
    vcr: &VcrContext,
    fork_config: Option<&ForkConfig>,
) -> Result<SessionOutcome> {
    let mut locals = SessionLocals {
        event_buffer: Vec::new(),
        pending_followups: Vec::new(),
        result_text: String::new(),
    };

    loop {
        let io_event: IoEvent = vcr
            .call("next_event", (), async |(): &()| io.next_event().await)
            .await?;
        match io_event {
            IoEvent::Claude(app_event) => {
                if input.is_active() && state.status == SessionStatus::Running {
                    locals.event_buffer.push(app_event);
                } else {
                    let outcome = process_claude_event(
                        app_event,
                        state,
                        renderer,
                        runner,
                        &mut locals,
                        vcr,
                        fork_config,
                    )
                    .await?;
                    if let Some(outcome) = outcome {
                        return Ok(outcome);
                    }
                }
            }
            IoEvent::Terminal(Event::Key(key_event)) => {
                let action = handle_session_key_event(
                    &key_event,
                    input,
                    renderer,
                    runner,
                    state,
                    &mut locals,
                    vcr,
                )
                .await?;
                match action {
                    LoopAction::Continue => {}
                    LoopAction::Return(outcome) => return Ok(outcome),
                }
            }
            IoEvent::Terminal(_) => {}
        }
    }
}

/// Flow control signals from key event handlers.
enum LoopAction {
    Continue,
    Return(SessionOutcome),
}

/// What happened during event buffer flush that requires caller action.
enum FlushResult {
    /// No special action needed.
    Continue,
    /// A pending followup was dequeued after a buffered Result.
    /// Caller should send it and set state to Running.
    Followup(String),
    /// A Result was flushed with no pending followups — session completed.
    Completed(String),
    /// The process exited during the flush.
    ProcessExited,
}

/// Handle a key event during an active session.
async fn handle_session_key_event<W: Write>(
    key_event: &KeyEvent,
    input: &mut InputHandler,
    renderer: &mut Renderer<W>,
    runner: &mut SessionRunner,
    state: &mut SessionState,
    locals: &mut SessionLocals,
    vcr: &VcrContext,
) -> Result<LoopAction> {
    let action = input.handle_key(key_event);
    match action {
        InputAction::Activated(c) => {
            renderer.begin_input_line();
            renderer.write_raw(&c.to_string());
        }
        InputAction::Submit(text, mode) => {
            let flush = flush_event_buffer(locals, state, renderer);
            // Completed is intentionally not special-cased here: if the session
            // completed during the flush, state is WaitingForInput and the match
            // below will send the user's text as a follow-up.
            if let Some(action) = handle_flush_result(flush, state, renderer, runner, vcr).await? {
                return Ok(action);
            }
            match mode {
                InputMode::Steering => {
                    renderer.render_steering_sent(&text);
                    vcr.call("send_message", text, async |t: &String| {
                        runner.send_message(t).await
                    })
                    .await?;
                }
                InputMode::FollowUp => {
                    if state.status == SessionStatus::WaitingForInput {
                        renderer.render_user_message(&text);
                        state.suppress_next_separator = true;
                        vcr.call("send_message", text, async |t: &String| {
                            runner.send_message(t).await
                        })
                        .await?;
                        state.status = SessionStatus::Running;
                    } else {
                        renderer.render_followup_queued(&text);
                        locals.pending_followups.push(text);
                    }
                }
            }
        }
        InputAction::ViewMessage(ref query) => {
            view_message(renderer, query);
            let flush = flush_event_buffer(locals, state, renderer);
            if let FlushResult::Completed(ref result_text) = flush {
                return Ok(LoopAction::Return(SessionOutcome::Completed {
                    result_text: result_text.clone(),
                }));
            }
            if let Some(action) = handle_flush_result(flush, state, renderer, runner, vcr).await? {
                return Ok(action);
            }
            // Don't re-activate input — it was deactivated when the user submitted
            // the :N command. The user returns to inactive state and can type a
            // character to start new input naturally (via Activated(c)).
        }
        InputAction::Interrupt => {
            runner.kill().await?;
            return Ok(LoopAction::Return(SessionOutcome::Interrupted));
        }
        InputAction::EndSession => {
            runner.close_input();
        }
        InputAction::Cancel => {
            let flush = flush_event_buffer(locals, state, renderer);
            if let FlushResult::Completed(ref result_text) = flush {
                return Ok(LoopAction::Return(SessionOutcome::Completed {
                    result_text: result_text.clone(),
                }));
            }
            if let Some(action) = handle_flush_result(flush, state, renderer, runner, vcr).await? {
                return Ok(action);
            }
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
async fn process_claude_event<W: Write>(
    event: AppEvent,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
    runner: &mut SessionRunner,
    locals: &mut SessionLocals,
    vcr: &VcrContext,
    fork_config: Option<&ForkConfig>,
) -> Result<Option<SessionOutcome>> {
    match event {
        AppEvent::Claude(inbound) => {
            // Capture result text early (needed for fork detection)
            if let InboundEvent::Result(ref result) = *inbound {
                locals.result_text.clone_from(&result.result);
            }

            // Detect fork tag in result text (live mode only — fork children
            // spawn real sessions, which isn't compatible with VCR replay).
            let fork_tasks = if vcr.is_live() {
                if let InboundEvent::Result(_) = *inbound {
                    fork_config.and_then(|_| fork::parse_fork_tag(&locals.result_text))
                } else {
                    None
                }
            } else {
                None
            };

            // Suppress the Done line if fork detected or followups pending
            let has_pending = !locals.pending_followups.is_empty() || fork_tasks.is_some();
            handle_inbound(&inbound, state, renderer, has_pending);

            // Run fork flow: spawn children, collect results, send reintegration
            if let Some(tasks) = fork_tasks {
                let session_id = state.session_id.clone().unwrap_or_default();
                // Safety: fork_tasks is only set when fork_config is Some
                let Some(fork_cfg) = fork_config else {
                    unreachable!("fork_tasks set without fork_config");
                };
                let msg = fork::run_fork(&session_id, tasks, fork_cfg, renderer).await?;
                runner.send_message(&msg).await?;
                state.suppress_next_separator = true;
                state.status = SessionStatus::Running;
                return Ok(None);
            }

            // If result and there's a pending follow-up, send it (FIFO)
            if matches!(*inbound, InboundEvent::Result(_)) {
                if locals.pending_followups.is_empty() {
                    return Ok(Some(SessionOutcome::Completed {
                        result_text: locals.result_text.clone(),
                    }));
                }
                let text = locals.pending_followups.remove(0);
                renderer.render_followup_sent(&text);
                state.suppress_next_separator = true;
                vcr.call("send_message", text, async |t: &String| {
                    runner.send_message(t).await
                })
                .await?;
                state.status = SessionStatus::Running;
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
///
/// Returns a `FlushResult` indicating whether the caller needs to take action:
/// sending a dequeued followup, handling a completion, or handling a process exit.
/// If multiple significant events are buffered, the last one wins (e.g. a
/// `ProcessExit` after a `Result` overrides the `Completed`/`Followup` result).
fn flush_event_buffer<W: Write>(
    locals: &mut SessionLocals,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
) -> FlushResult {
    let mut result = FlushResult::Continue;
    for event in locals.event_buffer.drain(..) {
        match event {
            AppEvent::Claude(inbound) => {
                let has_pending = !locals.pending_followups.is_empty();
                // Capture result text from buffered events too
                if let InboundEvent::Result(ref r) = *inbound {
                    locals.result_text.clone_from(&r.result);
                }
                handle_inbound(&inbound, state, renderer, has_pending);
                if matches!(*inbound, InboundEvent::Result(_)) {
                    if has_pending {
                        let text = locals.pending_followups.remove(0);
                        result = FlushResult::Followup(text);
                    } else {
                        result = FlushResult::Completed(locals.result_text.clone());
                    }
                }
            }
            AppEvent::ParseWarning(warning) => {
                renderer.render_warning(&warning);
            }
            AppEvent::ProcessExit(code) => {
                renderer.render_exit(code);
                state.status = SessionStatus::Ended;
                result = FlushResult::ProcessExited;
            }
        }
    }
    result
}

/// Handle the result of flushing the event buffer: send a dequeued followup
/// or return early on process exit.
async fn handle_flush_result<W: Write>(
    flush: FlushResult,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
    runner: &mut SessionRunner,
    vcr: &VcrContext,
) -> Result<Option<LoopAction>> {
    match flush {
        FlushResult::ProcessExited => Ok(Some(LoopAction::Return(SessionOutcome::ProcessExited))),
        FlushResult::Followup(text) => {
            renderer.render_followup_sent(&text);
            state.suppress_next_separator = true;
            vcr.call("send_message", text, async |t: &String| {
                runner.send_message(t).await
            })
            .await?;
            state.status = SessionStatus::Running;
            Ok(None)
        }
        FlushResult::Completed(_) | FlushResult::Continue => Ok(None),
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
pub async fn wait_for_followup<W: Write>(
    input: &mut InputHandler,
    renderer: &mut Renderer<W>,
    runner: &mut SessionRunner,
    state: &mut SessionState,
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<FollowUpAction> {
    match wait_for_text_input(input, renderer, io, vcr).await? {
        Some(text) => {
            state.suppress_next_separator = true;
            vcr.call("send_message", text, async |t: &String| {
                runner.send_message(t).await
            })
            .await?;
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
pub async fn wait_for_user_input<W: Write>(
    input: &mut InputHandler,
    renderer: &mut Renderer<W>,
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<Option<String>> {
    wait_for_text_input(input, renderer, io, vcr).await
}

/// Wait for user to type and submit text, or exit.
///
/// Shows the prompt, activates input, and loops on events.
/// Returns the submitted text, or None if the user interrupted/ended.
async fn wait_for_text_input<W: Write>(
    input: &mut InputHandler,
    renderer: &mut Renderer<W>,
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<Option<String>> {
    renderer.show_prompt();
    input.activate();

    loop {
        let io_event: IoEvent = vcr
            .call("next_event", (), async |(): &()| io.next_event().await)
            .await?;
        match io_event {
            IoEvent::Terminal(Event::Key(key_event)) => {
                let action = input.handle_key(&key_event);
                match action {
                    InputAction::Submit(text, _) => {
                        renderer.render_user_message(&text);
                        return Ok(Some(text));
                    }
                    InputAction::ViewMessage(ref query) => {
                        view_message(renderer, query);
                    }
                    InputAction::Cancel => {
                        renderer.show_prompt();
                        input.activate();
                    }
                    InputAction::Interrupt | InputAction::EndSession => {
                        return Ok(None);
                    }
                    InputAction::Activated(c) => {
                        renderer.begin_input_line();
                        renderer.write_raw(&c.to_string());
                    }
                    InputAction::None => {}
                }
            }
            IoEvent::Claude(AppEvent::ProcessExit(_)) => return Ok(None),
            IoEvent::Terminal(_) | IoEvent::Claude(_) => {}
        }
    }
}

/// Spawn a new Claude session via VCR.
pub async fn spawn_session(
    config: SessionConfig,
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<SessionRunner> {
    vcr.call("spawn", config, async |c: &SessionConfig| {
        let tx = io.replace_event_channel();
        SessionRunner::spawn(c.clone(), tx).await
    })
    .await
}

/// Open a message in $PAGER, looked up by label query (e.g. "3" or "2/1").
pub fn view_message<W: Write>(renderer: &mut Renderer<W>, query: &str) {
    use crate::display::renderer::format_message;

    let Some(content) = format_message(renderer.messages(), query) else {
        renderer.write_raw(&format!("No message {query}\r\n"));
        return;
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
            stdin.write_all(content.as_bytes()).ok();
        }
        // Close stdin so pager reads EOF
        child.stdin.take();
        child.wait().ok();
    }

    // Discard any keystrokes buffered in the kernel's terminal input queue
    // while the pager was active — prevents stale keys from leaking into the prompt.
    // SAFETY: tcflush on STDIN_FILENO with TCIFLUSH is a POSIX syscall that
    // discards buffered input bytes — no memory or resource safety concerns.
    unsafe { libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH) };
    terminal::enable_raw_mode().ok();
}
