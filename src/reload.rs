/// Message sent to the resumed session after a reload.
pub const RELOAD_RESUME_MESSAGE: &str =
    "Coven reloaded. Session resumed with fresh tool definitions. Continue where you left off.";

/// Build the system prompt fragment that teaches the model about reloading.
pub fn reload_system_prompt() -> &'static str {
    "To reload coven and pick up new configuration, emit a <reload> tag:\n\
     <reload>reason</reload>\n\
     This restarts coven and resumes your session with fresh tool definitions. \
     Use this after updating skills, MCP servers, or other config. \
     Your conversation context is preserved."
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
