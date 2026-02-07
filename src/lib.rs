use std::io::Write;

use display::renderer::Renderer;
use protocol::types::{AssistantContentBlock, InboundEvent, SystemEvent};
use session::state::{SessionState, SessionStatus};

pub mod display;
pub mod event;
pub mod protocol;
pub mod session;
pub mod vcr;

/// Handle an inbound Claude event, updating session state and rendering output.
pub fn handle_inbound<W: Write>(
    event: &InboundEvent,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
) {
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
                renderer.render_tool_result(result, u.message.as_ref());
            }
        }
        InboundEvent::Result(result) => {
            state.total_cost_usd = result.total_cost_usd;
            state.num_turns = result.num_turns;
            state.duration_ms = result.duration_ms;
            state.status = SessionStatus::WaitingForInput;
            let total_tokens = result.usage.as_ref().map(|u| {
                u.input_tokens
                    + u.output_tokens
                    + u.cache_read_input_tokens
                    + u.cache_creation_input_tokens
            });
            renderer.render_result(
                &result.subtype,
                result.total_cost_usd,
                result.duration_ms,
                result.num_turns,
                total_tokens,
            );
        }
    }
}
