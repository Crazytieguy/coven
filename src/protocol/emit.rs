use super::types::{OutboundMessage, OutboundMessageBody};

/// Format a user message as NDJSON for writing to claude's stdin.
pub fn format_user_message(text: &str) -> String {
    let msg = OutboundMessage {
        r#type: "user".to_string(),
        message: OutboundMessageBody {
            role: "user".to_string(),
            content: text.to_string(),
        },
    };
    // serde_json::to_string won't fail for this simple struct
    serde_json::to_string(&msg).expect("serialization of OutboundMessage should not fail")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_simple_message() {
        let json = format_user_message("hello");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "user");
        assert_eq!(parsed["message"]["role"], "user");
        assert_eq!(parsed["message"]["content"], "hello");
    }

    #[test]
    fn format_message_with_special_chars() {
        let json = format_user_message("hello \"world\"\nnewline");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["message"]["content"], "hello \"world\"\nnewline");
    }
}
