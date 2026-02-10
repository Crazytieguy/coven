/// Parse a `<fork>` tag from response text and return the task labels.
///
/// The tag contains a YAML-style list of task labels:
/// ```text
/// <fork>
/// - Refactor auth module
/// - Add tests for user API
/// </fork>
/// ```
pub fn parse_fork_tag(text: &str) -> Option<Vec<String>> {
    let open = "<fork>";
    let close = "</fork>";

    let start = text.find(open)?;
    let after_open = start + open.len();
    let end = text[after_open..].find(close)?;
    let inner = &text[after_open..after_open + end];

    let tasks: Vec<String> = inner
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.strip_prefix("- ").unwrap_or(line).trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if tasks.is_empty() {
        None
    } else {
        Some(tasks)
    }
}

/// Compose the XML reintegration message sent back to the parent session.
///
/// Each task's result (or error) is wrapped in a `<task>` element inside
/// `<fork-results>`, so the parent model can see what each child produced.
pub fn compose_reintegration_message(results: &[(String, Result<String, String>)]) -> String {
    use std::fmt::Write;

    let mut xml = String::from("<fork-results>\n");
    for (label, outcome) in results {
        // Escape label for XML attribute
        let safe_label = label.replace('"', "&quot;");
        match outcome {
            Ok(text) => {
                let _ = write!(
                    xml,
                    "<task label=\"{safe_label}\">\n<![CDATA[{text}]]>\n</task>\n"
                );
            }
            Err(err) => {
                let _ = write!(
                    xml,
                    "<task label=\"{safe_label}\" error=\"true\">\n<![CDATA[{err}]]>\n</task>\n"
                );
            }
        }
    }
    xml.push_str("</fork-results>");
    xml
}

/// Build the system prompt fragment that teaches the model about forking.
pub fn fork_system_prompt() -> &'static str {
    "To parallelize work, emit a <fork> tag containing a YAML list of short task labels:\n\
     <fork>\n\
     - Refactor auth module\n\
     - Add tests for user API\n\
     </fork>\n\
     Each fork inherits your full context and runs in parallel. You'll receive the results \
     in a <fork-results> message when all children complete."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fork_tag_basic() {
        let text = "Let me split this up.\n<fork>\n- Refactor auth\n- Add tests\n</fork>\nDone.";
        assert_eq!(
            parse_fork_tag(text),
            Some(vec!["Refactor auth".to_string(), "Add tests".to_string()])
        );
    }

    #[test]
    fn parse_fork_tag_single_task() {
        let text = "<fork>\n- Just one thing\n</fork>";
        assert_eq!(
            parse_fork_tag(text),
            Some(vec!["Just one thing".to_string()])
        );
    }

    #[test]
    fn parse_fork_tag_no_tag() {
        assert_eq!(parse_fork_tag("no fork here"), None);
    }

    #[test]
    fn parse_fork_tag_empty_list() {
        let text = "<fork>\n\n</fork>";
        assert_eq!(parse_fork_tag(text), None);
    }

    #[test]
    fn parse_fork_tag_partial() {
        let text = "<fork>\n- item\n but no closing tag";
        assert_eq!(parse_fork_tag(text), None);
    }

    #[test]
    fn parse_fork_tag_extra_whitespace() {
        let text = "<fork>\n  - spaced out  \n  - another  \n</fork>";
        assert_eq!(
            parse_fork_tag(text),
            Some(vec!["spaced out".to_string(), "another".to_string()])
        );
    }

    #[test]
    fn compose_reintegration_message_success() {
        let results = vec![
            ("Task A".to_string(), Ok("Result A".to_string())),
            ("Task B".to_string(), Ok("Result B".to_string())),
        ];
        let msg = compose_reintegration_message(&results);
        assert!(msg.starts_with("<fork-results>"));
        assert!(msg.ends_with("</fork-results>"));
        assert!(msg.contains("<task label=\"Task A\">"));
        assert!(msg.contains("<![CDATA[Result A]]>"));
        assert!(msg.contains("<task label=\"Task B\">"));
        assert!(msg.contains("<![CDATA[Result B]]>"));
    }

    #[test]
    fn compose_reintegration_message_with_error() {
        let results = vec![
            ("Good".to_string(), Ok("worked".to_string())),
            ("Bad".to_string(), Err("process crashed".to_string())),
        ];
        let msg = compose_reintegration_message(&results);
        assert!(msg.contains("<task label=\"Good\">"));
        assert!(msg.contains("<task label=\"Bad\" error=\"true\">"));
        assert!(msg.contains("<![CDATA[process crashed]]>"));
    }

    #[test]
    fn compose_reintegration_message_handles_angle_brackets() {
        let results = vec![(
            "Fix code".to_string(),
            Ok("Changed Vec<String> to Vec<&str>".to_string()),
        )];
        let msg = compose_reintegration_message(&results);
        assert!(msg.contains("<![CDATA[Changed Vec<String> to Vec<&str>]]>"));
    }

    #[test]
    fn compose_reintegration_message_escapes_label() {
        let results = vec![(
            "Fix \"quotes\"".to_string(),
            Ok("done".to_string()),
        )];
        let msg = compose_reintegration_message(&results);
        assert!(msg.contains("label=\"Fix &quot;quotes&quot;\""));
    }

    #[test]
    fn fork_system_prompt_contains_tag() {
        let prompt = fork_system_prompt();
        assert!(prompt.contains("<fork>"));
        assert!(prompt.contains("</fork>"));
        assert!(prompt.contains("<fork-results>"));
    }
}
