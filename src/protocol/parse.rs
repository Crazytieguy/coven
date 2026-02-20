use anyhow::Result;

use super::types::InboundEvent;

/// Extract the text between `<tag>` and `</tag>`.
///
/// Returns `None` if the tag pair is not found.
pub fn extract_tag_inner<'a>(text: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)?;
    let after_open = start + open.len();
    let rest = &text[after_open..];
    let mut depth = 1u32;
    let mut pos = 0;
    while depth > 0 {
        let next_open = rest[pos..].find(&open).map(|i| (pos + i, true));
        let next_close = rest[pos..].find(&close).map(|i| (pos + i, false));
        let next = match (next_open, next_close) {
            (Some(o), Some(c)) if o.0 < c.0 => o,
            (_, Some(c)) => c,
            (Some(o), None) => o,
            (None, None) => return None,
        };
        if next.1 {
            depth += 1;
            pos = next.0 + open.len();
        } else {
            depth -= 1;
            if depth == 0 {
                return Some(&rest[..next.0]);
            }
            pos = next.0 + close.len();
        }
    }
    None
}

/// Parse a single NDJSON line into an `InboundEvent`.
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn extract_tag_basic() {
        assert_eq!(
            extract_tag_inner("before <foo>content</foo> after", "foo"),
            Some("content")
        );
    }

    #[test]
    fn extract_tag_missing() {
        assert_eq!(extract_tag_inner("no tag here", "foo"), None);
    }

    #[test]
    fn extract_tag_unclosed() {
        assert_eq!(extract_tag_inner("<foo>content but no close", "foo"), None);
    }

    #[test]
    fn extract_tag_preserves_whitespace() {
        assert_eq!(
            extract_tag_inner("<t>  spaced  </t>", "t"),
            Some("  spaced  ")
        );
    }

    #[test]
    fn extract_tag_nested() {
        assert_eq!(
            extract_tag_inner("<break>outer <break>inner</break> after</break>", "break"),
            Some("outer <break>inner</break> after")
        );
    }

    #[test]
    fn extract_tag_two_separate_pairs() {
        assert_eq!(
            extract_tag_inner("<foo>first</foo> gap <foo>second</foo>", "foo"),
            Some("first")
        );
    }

    #[test]
    fn extract_tag_deeply_nested() {
        assert_eq!(
            extract_tag_inner("<t><t><t>deep</t></t></t>", "t"),
            Some("<t><t>deep</t></t>")
        );
    }

    #[test]
    fn parse_empty_line() {
        assert!(parse_line("").unwrap().is_none());
        assert!(parse_line("  \n").unwrap().is_none());
    }

    #[test]
    fn unknown_fields_dont_crash() {
        let line = r#"{"type":"result","subtype":"success","total_cost_usd":0.01,"num_turns":1,"duration_ms":100,"result":"ok","session_id":"x","unknown_field":"value","another":123}"#;
        assert!(parse_line(line).is_ok());
    }

    #[test]
    fn parse_rate_limit_event() {
        let line = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","resetsAt":1771545600,"rateLimitType":"seven_day","utilization":0.76,"isUsingOverage":false,"surpassedThreshold":0.75},"uuid":"e79d3169-e675-4aef-9400-8403f2237090","session_id":"bb1caa74-b643-4163-ba7d-8f6749891cc3"}"#;
        let event = parse_line(line).unwrap().unwrap();
        match event {
            InboundEvent::RateLimit(rl) => {
                assert_eq!(rl.rate_limit_info.status, "allowed_warning");
                assert_eq!(rl.rate_limit_info.rate_limit_type, "seven_day");
                assert!((rl.rate_limit_info.utilization - 0.76).abs() < f64::EPSILON);
                assert!(rl.rate_limit_info.is_warning());
            }
            other => panic!("Expected RateLimit, got {other:?}"),
        }
    }

    #[test]
    fn rate_limit_is_warning() {
        let line = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour","utilization":0.0},"uuid":"a","session_id":"b"}"#;
        let event = parse_line(line).unwrap().unwrap();
        match event {
            InboundEvent::RateLimit(rl) => {
                assert!(!rl.rate_limit_info.is_warning());
            }
            other => panic!("Expected RateLimit, got {other:?}"),
        }
    }
}
