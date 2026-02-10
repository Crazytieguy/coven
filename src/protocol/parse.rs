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
    let end = text[after_open..].find(&close)?;
    Some(&text[after_open..after_open + end])
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
    fn parse_empty_line() {
        assert!(parse_line("").unwrap().is_none());
        assert!(parse_line("  \n").unwrap().is_none());
    }

    #[test]
    fn unknown_fields_dont_crash() {
        let line = r#"{"type":"result","subtype":"success","total_cost_usd":0.01,"num_turns":1,"duration_ms":100,"result":"ok","session_id":"x","unknown_field":"value","another":123}"#;
        assert!(parse_line(line).is_ok());
    }
}
