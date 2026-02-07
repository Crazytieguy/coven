use anyhow::Result;

use super::types::InboundEvent;

/// Parse a single NDJSON line into an InboundEvent.
///
/// Returns `Ok(None)` for empty lines.
/// Returns `Err` for malformed JSON (caller should warn, not crash).
pub fn parse_line(line: &str) -> Result<Option<InboundEvent>> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(None);
    }
    let event: InboundEvent = serde_json::from_str(line)?;
    Ok(Some(event))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_line() {
        assert!(parse_line("").unwrap().is_none());
        assert!(parse_line("  \n").unwrap().is_none());
    }

    #[test]
    fn parse_init_event() {
        let line = r#"{"type":"system","subtype":"init","session_id":"abc123","model":"claude-sonnet-4-20250514","tools":[]}"#;
        let event = parse_line(line).unwrap().unwrap();
        match event {
            InboundEvent::System(super::super::types::SystemEvent::Init(init)) => {
                assert_eq!(init.session_id, "abc123");
                assert_eq!(init.model, "claude-sonnet-4-20250514");
            }
            other => panic!("Expected System/Init, got {other:?}"),
        }
    }

    #[test]
    fn parse_result_event() {
        let line = r#"{"type":"result","subtype":"success","total_cost_usd":0.03,"num_turns":3,"duration_ms":12400,"result":"Done","session_id":"abc123"}"#;
        let event = parse_line(line).unwrap().unwrap();
        match event {
            InboundEvent::Result(result) => {
                assert_eq!(result.subtype, "success");
                assert!((result.total_cost_usd - 0.03).abs() < f64::EPSILON);
                assert_eq!(result.num_turns, 3);
            }
            other => panic!("Expected Result, got {other:?}"),
        }
    }

    #[test]
    fn parse_stream_event() {
        let line = r#"{"type":"stream_event","event":"content_block_delta","delta":{"type":"text_delta","text":"hello"}}"#;
        let event = parse_line(line).unwrap().unwrap();
        match event {
            InboundEvent::StreamEvent(se) => {
                assert_eq!(se.event, "content_block_delta");
                let delta = se.delta.unwrap();
                assert_eq!(delta.r#type, "text_delta");
                assert_eq!(delta.text.unwrap(), "hello");
            }
            other => panic!("Expected StreamEvent, got {other:?}"),
        }
    }

    #[test]
    fn parse_assistant_event() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello!"},{"type":"tool_use","id":"tu1","name":"Bash","input":{"command":"ls"}}]}}"#;
        let event = parse_line(line).unwrap().unwrap();
        match event {
            InboundEvent::Assistant(msg) => {
                assert_eq!(msg.message.content.len(), 2);
            }
            other => panic!("Expected Assistant, got {other:?}"),
        }
    }

    #[test]
    fn parse_user_tool_result() {
        let line = r#"{"type":"user","tool_use_result":{"tool_use_id":"tu1","name":"Bash","is_error":false}}"#;
        let event = parse_line(line).unwrap().unwrap();
        match event {
            InboundEvent::User(u) => {
                let result = u.tool_use_result.unwrap();
                assert_eq!(result.name, "Bash");
                assert!(!result.is_error);
            }
            other => panic!("Expected User, got {other:?}"),
        }
    }

    #[test]
    fn unknown_fields_dont_crash() {
        let line = r#"{"type":"result","subtype":"success","total_cost_usd":0.01,"num_turns":1,"duration_ms":100,"result":"ok","session_id":"x","unknown_field":"value","another":123}"#;
        assert!(parse_line(line).is_ok());
    }
}
