use super::types::{OutboundMessage, OutboundMessageBody};

/// Format a user message as NDJSON for writing to claude's stdin.
///
/// # Errors
///
/// Returns an error if JSON serialization fails (should not happen in practice).
pub fn format_user_message(text: &str) -> serde_json::Result<String> {
    let msg = OutboundMessage {
        r#type: "user".to_string(),
        message: OutboundMessageBody {
            role: "user".to_string(),
            content: text.to_string(),
        },
    };
    serde_json::to_string(&msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_simple_message() {
        let json = format_user_message("hello").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["message"]["role"], "user");
        assert_eq!(parsed["message"]["content"], "hello");
    }

    #[test]
    fn format_message_with_special_chars() {
        let json = format_user_message("hello \"world\"\nnewline").unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["message"]["content"], "hello \"world\"\nnewline");
    }
}
