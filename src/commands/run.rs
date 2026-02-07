use std::process::Command as StdCommand;

use anyhow::Result;
use crossterm::event::{Event, EventStream};
use crossterm::terminal;
use futures::StreamExt;
use tokio::sync::mpsc;

use crate::display::input::{InputAction, InputHandler};
use crate::display::renderer::Renderer;
use crate::event::{AppEvent, InputMode};
use crate::protocol::types::{AssistantContentBlock, InboundEvent, SystemEvent};
use crate::session::runner::{SessionConfig, SessionRunner};
use crate::session::state::{SessionState, SessionStatus};

/// Run a single interactive session.
pub async fn run(prompt: Option<String>, extra_args: Vec<String>) -> Result<()> {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

    let has_prompt = prompt.is_some();

    let config = SessionConfig {
        prompt,
        extra_args,
        append_system_prompt: None,
    };

    let mut renderer = Renderer::new();
    let mut input = InputHandler::new();
    let mut state = SessionState::default();
    let mut runner: Option<SessionRunner> = None;

    // Events buffered while user is typing mid-stream
    let mut event_buffer: Vec<AppEvent> = Vec::new();

    // If no prompt, show prompt first for user to type
    if !has_prompt {
        terminal::enable_raw_mode()?;
        renderer.show_prompt();
        input.activate();
    }

    // Spawn claude if we have a prompt
    if has_prompt {
        terminal::enable_raw_mode()?;
        runner = Some(SessionRunner::spawn(config, event_tx.clone()).await?);
    }

    // Terminal event reader
    let mut term_events = EventStream::new();

    // Deferred follow-up message
    let mut pending_followup: Option<String> = None;

    loop {
        tokio::select! {
            // Claude process events
            event = event_rx.recv() => {
                match event {
                    Some(app_event) => {
                        if input.is_active() && state.status == SessionStatus::Running {
                            // Buffer events while user is typing mid-stream
                            event_buffer.push(app_event);
                        } else {
                            process_claude_event(
                                app_event,
                                &mut state,
                                &mut renderer,
                                &mut runner,
                                &mut input,
                                &mut pending_followup,
                            ).await?;
                            if state.status == SessionStatus::Ended {
                                break;
                            }
                        }
                    }
                    None => break,
                }
            }

            // Terminal key events
            term_event = term_events.next() => {
                match term_event {
                    Some(Ok(Event::Key(key_event))) => {
                        // If no session yet (no-prompt mode), handle initial input
                        if runner.is_none() {
                            let action = input.handle_key(&key_event);
                            match action {
                                InputAction::Submit(text, _) => {
                                    // Start session with this prompt
                                    let config = SessionConfig {
                                        prompt: Some(text),
                                        extra_args: vec![],
                                        append_system_prompt: None,
                                    };
                                    runner = Some(
                                        SessionRunner::spawn(config, event_tx.clone()).await?
                                    );
                                    state.status = SessionStatus::Running;
                                }
                                InputAction::Interrupt => break,
                                InputAction::EndSession => break,
                                InputAction::Cancel => {
                                    renderer.show_prompt();
                                    input.activate();
                                }
                                _ => {}
                            }
                            continue;
                        }

                        let action = input.handle_key(&key_event);
                        match action {
                            InputAction::Submit(text, mode) => {
                                // Flush buffered events before handling input
                                flush_event_buffer(
                                    &mut event_buffer,
                                    &mut state,
                                    &mut renderer,
                                ).await;

                                match mode {
                                    InputMode::Steering => {
                                        // Send immediately
                                        if let Some(ref mut r) = runner {
                                            r.send_message(&text).await?;
                                        }
                                    }
                                    InputMode::FollowUp => {
                                        if state.status == SessionStatus::WaitingForInput {
                                            // Send now (session is idle)
                                            if let Some(ref mut r) = runner {
                                                r.send_message(&text).await?;
                                                state.status = SessionStatus::Running;
                                            }
                                        } else {
                                            // Buffer for later
                                            pending_followup = Some(text);
                                        }
                                    }
                                }
                            }
                            InputAction::ViewMessage(n) => {
                                view_message(&mut renderer, n);
                                // Re-show prompt
                                renderer.show_prompt();
                                input.activate();
                            }
                            InputAction::Interrupt => {
                                if let Some(ref mut r) = runner {
                                    r.kill().await?;
                                }
                                break;
                            }
                            InputAction::EndSession => {
                                if let Some(ref mut r) = runner {
                                    r.close_input().await;
                                }
                            }
                            InputAction::Cancel => {
                                // Flush buffered events on cancel
                                flush_event_buffer(
                                    &mut event_buffer,
                                    &mut state,
                                    &mut renderer,
                                ).await;

                                if state.status == SessionStatus::WaitingForInput {
                                    renderer.show_prompt();
                                    input.activate();
                                }
                            }
                            InputAction::None => {}
                        }
                    }
                    Some(Ok(_)) => {} // mouse, resize, etc
                    Some(Err(_)) => break,
                    None => break,
                }
            }
        }
    }

    // Cleanup
    terminal::disable_raw_mode()?;

    if let Some(ref mut r) = runner {
        r.close_input().await;
        let _ = r.wait().await;
    }

    Ok(())
}

/// Process a single claude event (not buffered).
async fn process_claude_event(
    event: AppEvent,
    state: &mut SessionState,
    renderer: &mut Renderer,
    runner: &mut Option<SessionRunner>,
    input: &mut InputHandler,
    pending_followup: &mut Option<String>,
) -> Result<()> {
    match event {
        AppEvent::Claude(inbound) => {
            handle_inbound(&inbound, state, renderer);

            // If result and there's a pending follow-up, send it
            if matches!(*inbound, InboundEvent::Result(_)) {
                if let Some(text) = pending_followup.take() {
                    if let Some(r) = runner {
                        r.send_message(&text).await?;
                        state.status = SessionStatus::Running;
                    }
                } else {
                    // Show prompt for next input
                    renderer.show_prompt();
                    input.activate();
                }
            }
        }
        AppEvent::ParseWarning(warning) => {
            renderer.render_warning(&warning);
        }
        AppEvent::ProcessExit(code) => {
            renderer.render_exit(code);
            state.status = SessionStatus::Ended;
        }
        _ => {}
    }
    Ok(())
}

/// Flush all buffered events through the renderer.
async fn flush_event_buffer(
    buffer: &mut Vec<AppEvent>,
    state: &mut SessionState,
    renderer: &mut Renderer,
) {
    for event in buffer.drain(..) {
        match event {
            AppEvent::Claude(inbound) => {
                handle_inbound(&inbound, state, renderer);
            }
            AppEvent::ParseWarning(warning) => {
                renderer.render_warning(&warning);
            }
            AppEvent::ProcessExit(code) => {
                renderer.render_exit(code);
                state.status = SessionStatus::Ended;
            }
            _ => {}
        }
    }
}

fn handle_inbound(event: &InboundEvent, state: &mut SessionState, renderer: &mut Renderer) {
    match event {
        InboundEvent::System(SystemEvent::Init(init)) => {
            state.session_id = Some(init.session_id.clone());
            state.model = Some(init.model.clone());
            state.status = SessionStatus::Running;
            renderer.render_session_header(&init.session_id, &init.model);
        }
        InboundEvent::System(SystemEvent::Other) => {}
        InboundEvent::StreamEvent(se) => {
            renderer.handle_stream_event(se);
        }
        InboundEvent::Assistant(msg) => {
            if msg.parent_tool_use_id.is_some() {
                // Subagent tool call — render indented
                for block in &msg.message.content {
                    if let AssistantContentBlock::ToolUse { name, input, .. } = block {
                        renderer.render_subagent_tool_call(name, input);
                    }
                }
            }
        }
        InboundEvent::User(u) => {
            if u.parent_tool_use_id.is_some() {
                if let Some(ref message) = u.message {
                    renderer.render_subagent_tool_result(message);
                }
            } else if let Some(ref result) = u.tool_use_result {
                renderer.render_tool_result(result);
            }
        }
        InboundEvent::Result(result) => {
            state.total_cost_usd = result.total_cost_usd;
            state.num_turns = result.num_turns;
            state.duration_ms = result.duration_ms;
            state.status = SessionStatus::WaitingForInput;
            renderer.render_result(
                &result.subtype,
                result.total_cost_usd,
                result.duration_ms,
                result.num_turns,
            );
        }
    }
}

/// Open message N in $PAGER.
fn view_message(renderer: &mut Renderer, n: usize) {
    let messages = renderer.messages();
    if n == 0 || n > messages.len() {
        renderer.write_raw(&format!("No message {n}\r\n"));
        return;
    }

    let msg = &messages[n - 1];
    let content = format!("{}\n\n{}", msg.label, msg.content);

    // Temporarily leave raw mode for pager
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
