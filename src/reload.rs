use std::io::Write;

use anyhow::Result;

use crate::display::renderer::Renderer;
use crate::session::runner::{SessionConfig, SessionRunner};
use crate::session::state::SessionState;
use crate::vcr::{Io, VcrContext};

/// Message sent to the resumed session after a reload.
pub const RELOAD_RESUME_MESSAGE: &str =
    "Claude reloaded with fresh tool definitions. Continue where you left off.";

/// Build the system prompt fragment that teaches the model about reloading.
pub fn reload_system_prompt() -> &'static str {
    "To pick up new configuration, emit a <reload> tag:\n\
     <reload>reason</reload>\n\
     This restarts the claude process with fresh tool definitions while preserving your session. \
     Use this after updating skills, MCP servers, or other config."
}

/// Spawn a resumed session after a reload, returning the new runner and state.
///
/// Writes the `[reloading claude...]` status line, creates a resume config from
/// the base session config, spawns the new session, and returns a fresh
/// `SessionState` with the session ID preserved.
pub async fn spawn_reload_session<W: Write>(
    session_id: String,
    base_config: &SessionConfig,
    renderer: &mut Renderer<W>,
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<(SessionRunner, SessionState)> {
    renderer.write_raw("[reloading claude...]\r\n");
    let resume_cfg = base_config.resume_with(RELOAD_RESUME_MESSAGE.to_string(), session_id.clone());
    let runner = crate::session::event_loop::spawn_session(resume_cfg, io, vcr).await?;
    let state = SessionState {
        session_id: Some(session_id),
        ..Default::default()
    };
    Ok((runner, state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_contains_tag() {
        let prompt = reload_system_prompt();
        assert!(prompt.contains("<reload>"));
        assert!(prompt.contains("</reload>"));
    }

    #[test]
    fn resume_message_not_empty() {
        assert!(!RELOAD_RESUME_MESSAGE.is_empty());
    }
}
