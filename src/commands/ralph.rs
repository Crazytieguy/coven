use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use crossterm::terminal;
use futures::StreamExt;
use tokio::sync::mpsc;

use crate::display::renderer::Renderer;
use crate::event::AppEvent;
use crate::protocol::types::{AssistantContentBlock, InboundEvent, SystemEvent};
use crate::session::runner::{SessionConfig, SessionRunner};
use crate::session::state::{SessionState, SessionStatus};

pub struct RalphConfig {
    pub prompt: String,
    pub iterations: u32,
    pub break_tag: String,
    pub no_break: bool,
    pub extra_args: Vec<String>,
}

/// Run ralph loop mode.
pub async fn ralph(config: RalphConfig) -> Result<()> {
    terminal::enable_raw_mode()?;

    // Install panic hook for terminal cleanup
    let mut renderer = Renderer::new();
    let mut total_cost = 0.0;
    let mut iteration = 0;

    let mut term_events = EventStream::new();

    let system_prompt = if config.no_break {
        "You are running in a loop where each iteration starts a fresh session but the filesystem \
         persists."
            .to_string()
    } else {
        SessionRunner::ralph_system_prompt(&config.break_tag)
    };

    loop {
        iteration += 1;

        // Check iteration limit
        if config.iterations > 0 && iteration > config.iterations {
            renderer.write_raw(&format!(
                "\r\nReached iteration limit ({})\r\n",
                config.iterations
            ));
            break;
        }

        // Iteration header
        renderer.write_raw(&format!("\r\n--- Iteration {iteration} ---\r\n\r\n"));

        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

        let session_config = SessionConfig {
            prompt: Some(config.prompt.clone()),
            extra_args: config.extra_args.clone(),
            append_system_prompt: Some(system_prompt.clone()),
        };

        let mut runner = SessionRunner::spawn(session_config, event_tx).await?;
        let mut state = SessionState::default();
        let mut result_text = String::new();

        // Process events for this iteration
        let mut interrupted = false;
        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    match event {
                        Some(AppEvent::Claude(inbound)) => {
                            handle_inbound(&inbound, &mut state, &mut renderer);

                            // Capture result text for break tag scanning
                            if let InboundEvent::Result(ref result) = *inbound {
                                result_text.clone_from(&result.result);
                                total_cost += result.total_cost_usd;
                            }

                            if matches!(*inbound, InboundEvent::Result(_) | InboundEvent::User(_))
                                && state.status == SessionStatus::WaitingForInput
                            {
                                break;
                            }
                        }
                        Some(AppEvent::ParseWarning(warning)) => {
                            renderer.render_warning(&warning);
                        }
                        Some(AppEvent::ProcessExit(code)) => {
                            renderer.render_exit(code);
                            break;
                        }
                        Some(_) => {}
                        None => break,
                    }
                }
                term_event = term_events.next() => {
                    if let Some(Ok(Event::Key(key_event))) = term_event
                        && matches!(key_event.code,
                            KeyCode::Char('c' | 'd')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL)
                        )
                    {
                        runner.kill().await?;
                        interrupted = true;
                        break;
                    }
                }
            }
        }

        if interrupted {
            break;
        }

        runner.close_input();
        let _ = runner.wait().await;

        // Show running cost
        renderer.write_raw(&format!("  Total cost: ${total_cost:.2}\r\n"));

        // Check for break tag
        if !config.no_break
            && let Some(reason) = scan_break_tag(&result_text, &config.break_tag)
        {
            renderer.write_raw(&format!("\r\nLoop complete: {reason}\r\n"));
            break;
        }
    }

    terminal::disable_raw_mode()?;
    Ok(())
}

/// Scan response text for `<tag>reason</tag>` and return the reason if found.
fn scan_break_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");

    let start = text.find(&open)?;
    let after_open = start + open.len();
    let end = text[after_open..].find(&close)?;
    let reason = text[after_open..after_open + end].trim().to_string();
    Some(reason)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_break_tag_found() {
        let text = "I've completed the task. <break>All bugs are fixed.</break> Done.";
        assert_eq!(
            scan_break_tag(text, "break"),
            Some("All bugs are fixed.".to_string())
        );
    }

    #[test]
    fn scan_break_tag_custom() {
        let text = "Done! <done>Everything works</done>";
        assert_eq!(
            scan_break_tag(text, "done"),
            Some("Everything works".to_string())
        );
    }

    #[test]
    fn scan_break_tag_not_found() {
        let text = "Still working on the bugs.";
        assert_eq!(scan_break_tag(text, "break"), None);
    }

    #[test]
    fn scan_break_tag_partial() {
        let text = "Found <break> but no closing tag";
        assert_eq!(scan_break_tag(text, "break"), None);
    }
}
