use serde_json::Value;

/// Format a tool call's input for the `:N` detail view, dispatching on tool name.
/// Returns `None` for unknown tools (falls back to pretty JSON).
/// NOTE: When adding a tool here, also update [`format_tool_detail`].
pub fn format_tool_view(tool_name: &str, input: &Value) -> Option<String> {
    match tool_name {
        "Read" => {
            let path = get_str(input, "file_path")?;
            let offset = input.get("offset").and_then(Value::as_u64);
            let limit = input.get("limit").and_then(Value::as_u64);
            Some(match (offset, limit) {
                (Some(o), Some(l)) => format!("{path} (offset: {o}, limit: {l})"),
                (Some(o), None) => format!("{path} (offset: {o})"),
                (None, Some(l)) => format!("{path} (limit: {l})"),
                (None, None) => path.to_string(),
            })
        }
        "Edit" => {
            let path = get_str(input, "file_path")?;
            let old = get_str(input, "old_string").unwrap_or("");
            let new = get_str(input, "new_string").unwrap_or("");
            let mut lines = vec![path.to_string()];
            for line in old.lines() {
                lines.push(format!("\x1b[31m- {line}\x1b[0m"));
            }
            for line in new.lines() {
                lines.push(format!("\x1b[32m+ {line}\x1b[0m"));
            }
            Some(lines.join("\n"))
        }
        "Write" => {
            let path = get_str(input, "file_path")?;
            let content = get_str(input, "content").unwrap_or("");
            let mut lines = vec![path.to_string()];
            for (i, line) in content.lines().enumerate() {
                lines.push(format!("{:>4}  {line}", i + 1));
            }
            Some(lines.join("\n"))
        }
        "Bash" => {
            let cmd = get_str(input, "command")?;
            let timeout = input.get("timeout").and_then(Value::as_u64);
            match timeout {
                Some(t) => Some(format!("$ {cmd}\n\ntimeout: {t}ms")),
                None => Some(format!("$ {cmd}")),
            }
        }
        "Glob" => {
            let pattern = get_str(input, "pattern")?;
            match get_str(input, "path") {
                Some(path) => Some(format!("{pattern}  in {path}")),
                None => Some(pattern.to_string()),
            }
        }
        "Grep" => {
            let pattern = get_str(input, "pattern")?;
            match get_str(input, "path") {
                Some(path) => Some(format!("/{pattern}/  in {path}")),
                None => Some(format!("/{pattern}/")),
            }
        }
        "WebFetch" => {
            let url = get_str(input, "url")?;
            match get_str(input, "prompt") {
                Some(prompt) => Some(format!("{url}\n\n{prompt}")),
                None => Some(url.to_string()),
            }
        }
        "WebSearch" => {
            let query = get_str(input, "query")?;
            Some(query.to_string())
        }
        "Task" => {
            let desc = get_str(input, "description")?;
            let header = match get_str(input, "subagent_type") {
                Some(agent_type) => format!("[{agent_type}] {desc}"),
                None => desc.to_string(),
            };
            match get_str(input, "prompt") {
                Some(prompt) => Some(format!("{header}\n\n{prompt}")),
                None => Some(header),
            }
        }
        _ => None,
    }
}

/// Format a compact one-liner for the streaming tool call display.
/// NOTE: When adding a tool here, also update [`format_tool_view`].
pub fn format_tool_detail(name: &str, input: &Value) -> String {
    match name {
        "Read" => get_str(input, "file_path").unwrap_or_default().to_string(),
        "Edit" => {
            let path = get_str(input, "file_path").unwrap_or_default();
            let old_lines = get_str(input, "old_string").map_or(0, |s| s.lines().count());
            let new_lines = get_str(input, "new_string").map_or(0, |s| s.lines().count());
            if old_lines > 0 || new_lines > 0 {
                format!("(+{new_lines}/-{old_lines})  {path}")
            } else {
                path.to_string()
            }
        }
        "Write" => {
            let path = get_str(input, "file_path").unwrap_or_default();
            match get_str(input, "content").map(|c| c.lines().count()) {
                Some(count) => format!("(+{count})  {path}"),
                None => path.to_string(),
            }
        }
        "Glob" => get_str(input, "pattern").unwrap_or_default().to_string(),
        "Grep" => {
            let pattern = get_str(input, "pattern").unwrap_or_default();
            let path = get_str(input, "path").unwrap_or_default();
            if path.is_empty() {
                pattern.to_string()
            } else {
                format!("{pattern}  {path}")
            }
        }
        "Bash" => {
            let cmd = get_str(input, "command").unwrap_or_default();
            let timeout = input.get("timeout").and_then(Value::as_u64);
            match timeout {
                Some(t) => format!("{} (timeout: {t}ms)", first_line(cmd)),
                None => first_line(cmd).to_string(),
            }
        }
        "Task" => get_str(input, "description")
            .unwrap_or_default()
            .to_string(),
        "WebFetch" => get_str(input, "url").unwrap_or_default().to_string(),
        "WebSearch" => get_str(input, "query").unwrap_or_default().to_string(),
        _ => {
            // For MCP/other tools: show first string field value
            if let Value::Object(map) = input {
                for (_, v) in map {
                    if let Value::String(s) = v {
                        return first_line(s).to_string();
                    }
                }
            }
            String::new()
        }
    }
}

fn get_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// Extract the first line of a string (no truncation).
pub(crate) fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn format_tool_detail_edit_with_removals() {
        let input = serde_json::json!({
            "file_path": "/src/main.rs",
            "old_string": "line1\nline2\nline3",
            "new_string": "line1"
        });
        assert_eq!(format_tool_detail("Edit", &input), "(+1/-3)  /src/main.rs");
    }

    #[test]
    fn format_tool_detail_write_trailing_newline() {
        // str::lines() doesn't count a trailing newline as an extra line
        let input = serde_json::json!({
            "file_path": "/hello.txt",
            "content": "single line\n"
        });
        assert_eq!(format_tool_detail("Write", &input), "(+1)  /hello.txt");
    }

    #[test]
    fn format_tool_detail_write_no_content() {
        let input = serde_json::json!({"file_path": "/empty.txt"});
        assert_eq!(format_tool_detail("Write", &input), "/empty.txt");
    }

    #[test]
    fn format_tool_detail_grep_with_path() {
        let input = serde_json::json!({"pattern": "fn main", "path": "/src"});
        assert_eq!(format_tool_detail("Grep", &input), "fn main  /src");
    }

    #[test]
    fn format_tool_detail_web_fetch() {
        let input = serde_json::json!({"url": "https://docs.rs/tokio"});
        assert_eq!(
            format_tool_detail("WebFetch", &input),
            "https://docs.rs/tokio"
        );
    }

    #[test]
    fn format_tool_detail_web_search() {
        let input = serde_json::json!({"query": "rust async runtime"});
        assert_eq!(
            format_tool_detail("WebSearch", &input),
            "rust async runtime"
        );
    }

    #[test]
    fn format_tool_detail_unknown_tool() {
        let input = serde_json::json!({"some_key": "some_value"});
        assert_eq!(format_tool_detail("CustomTool", &input), "some_value");
    }

    #[test]
    fn format_tool_detail_unknown_tool_empty() {
        let input = serde_json::json!({});
        assert_eq!(format_tool_detail("CustomTool", &input), "");
    }
}
