use anyhow::Result;

use super::types::InboundEvent;

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
