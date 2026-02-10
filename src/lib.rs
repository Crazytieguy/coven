use std::io::Write;

use display::renderer::Renderer;
use protocol::types::{AssistantContentBlock, InboundEvent, SystemEvent};
use session::state::{SessionState, SessionStatus};

pub mod agents;
pub mod commands;
pub mod dispatch;
pub mod display;
pub mod event;
pub mod fork;
pub mod protocol;
pub mod session;
pub mod vcr;
pub mod worker_state;
pub mod worktree;

/// Handle an inbound Claude event, updating session state and rendering output.
///
/// When `has_pending_followups` is true, Result events update state but skip
/// rendering the Done line — the follow-up will continue the conversation.
pub fn handle_inbound<W: Write>(
    event: &InboundEvent,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
    has_pending_followups: bool,
) {
    match event {
        InboundEvent::System(SystemEvent::Init(init)) => {
            let same_session = state.session_id.as_deref() == Some(&init.session_id);
            state.session_id = Some(init.session_id.clone());
            state.model = Some(init.model.clone());
            state.status = SessionStatus::Running;
            if same_session {
                if state.suppress_next_separator {
                    state.suppress_next_separator = false;
                } else {
                    renderer.render_turn_separator();
                }
            } else {
                state.suppress_next_separator = false;
                renderer.render_session_header(&init.session_id, &init.model);
            }
        }
        InboundEvent::System(SystemEvent::Other) => {}
        InboundEvent::StreamEvent(se) => {
            renderer.handle_stream_event(se);
        }
        InboundEvent::Assistant(msg) => {
            if let Some(ref parent_id) = msg.parent_tool_use_id {
                for block in &msg.message.content {
                    if let AssistantContentBlock::ToolUse { name, input, .. } = block {
                        renderer.render_subagent_tool_call(name, input, parent_id);
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
                renderer.render_tool_result(result, u.message.as_ref());
            }
        }
        InboundEvent::Result(result) => {
            state.total_cost_usd = result.total_cost_usd;
            state.num_turns = result.num_turns;
            state.duration_ms = result.duration_ms;
            state.status = SessionStatus::WaitingForInput;
            if !has_pending_followups {
                renderer.render_result(
                    &result.subtype,
                    result.total_cost_usd,
                    result.duration_ms,
                    result.num_turns,
                );
            }
        }
    }
}
