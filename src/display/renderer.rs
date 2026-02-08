use std::io::{self, Write};

use crossterm::queue;
use crossterm::style::Print;
use serde_json::Value;
use unicode_width::UnicodeWidthChar;

use super::theme;
use crate::protocol::types::StreamEvent;

/// Stores a completed message for later viewing via `:N`.
#[derive(Debug)]
pub struct StoredMessage {
    pub label: String,
    pub content: String,
    /// Tool result text, attached when the result arrives.
    pub result: Option<String>,
}

/// Display configuration for the renderer.
#[derive(Default)]
pub struct RendererConfig {
    /// Whether to stream thinking text inline instead of collapsing.
    pub show_thinking: bool,
}

/// Tracks rendering state and produces colored terminal output.
pub struct Renderer<W: Write = io::Stdout> {
    /// Current content block type being streamed.
    current_block: Option<BlockKind>,
    /// Whether we're mid-line in text streaming.
    text_streaming: bool,
    /// Numbered messages for `:N` viewing.
    messages: Vec<StoredMessage>,
    /// Tool use counter for numbering.
    tool_counter: usize,
    /// The tool currently in progress (name + input).
    current_tool: Option<(String, Value)>,
    /// Accumulated thinking text for the current thinking block.
    current_thinking: Option<String>,
    /// Whether a tool call line is still open (no \r\n yet), awaiting its result.
    tool_line_open: bool,
    /// Whether the last tool call was a subagent (indented).
    last_tool_is_subagent: bool,
    /// Display configuration.
    config: RendererConfig,
    /// Writer for output.
    out: W,
}

#[derive(Debug, Clone, PartialEq)]
enum BlockKind {
    Text,
    ToolUse,
    Thinking,
}

impl Default for Renderer<io::Stdout> {
    fn default() -> Self {
        Self {
            current_block: None,
            text_streaming: false,
            messages: Vec::new(),
            tool_counter: 0,
            current_tool: None,
            current_thinking: None,
            tool_line_open: false,
            last_tool_is_subagent: false,
            config: RendererConfig::default(),
            out: io::stdout(),
        }
    }
}

impl Renderer<io::Stdout> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<W: Write> Renderer<W> {
    pub fn with_writer(writer: W) -> Self {
        Self {
            current_block: None,
            text_streaming: false,
            messages: Vec::new(),
            tool_counter: 0,
            current_tool: None,
            current_thinking: None,
            tool_line_open: false,
            last_tool_is_subagent: false,
            config: RendererConfig::default(),
            out: writer,
        }
    }

    pub fn set_show_thinking(&mut self, show: bool) {
        self.config.show_thinking = show;
    }

    pub fn messages(&self) -> &[StoredMessage] {
        &self.messages
    }

    // --- Session lifecycle ---

    pub fn render_help(&mut self) {
        let help = ":N view message · type to steer · Alt+Enter follow up · Ctrl+D exit";
        queue!(self.out, Print(theme::dim().apply(help)), Print("\r\n")).ok();
        self.out.flush().ok();
    }

    pub fn render_session_header(&mut self, session_id: &str, model: &str) {
        let header = format!("Session {session_id} ({model})");
        queue!(self.out, Print(theme::dim().apply(header)), Print("\r\n")).ok();
        queue!(self.out, Print("\r\n")).ok();
        self.out.flush().ok();
    }

    pub fn render_turn_separator(&mut self) {
        queue!(self.out, Print(theme::dim().apply("---")), Print("\r\n")).ok();
        queue!(self.out, Print("\r\n")).ok();
        self.out.flush().ok();
    }

    pub fn render_result(
        &mut self,
        subtype: &str,
        cost: f64,
        duration_ms: u64,
        num_turns: u32,
        total_tokens: Option<u64>,
    ) {
        self.close_tool_line();
        self.finish_current_block();
        // Round to tenths of a second (add 50ms to round instead of truncate)
        let rounded = duration_ms + 50;
        let whole_secs = rounded / 1000;
        let tenths = (rounded % 1000) / 100;

        let label = if subtype == "success" {
            "Done"
        } else {
            "Error"
        };
        let tokens_str = match total_tokens {
            Some(t) => format!(" · {}k tokens", t / 1000),
            None => String::new(),
        };
        let turn_word = if num_turns == 1 { "turn" } else { "turns" };
        let stats =
            format!("  ${cost:.2} · {whole_secs}.{tenths}s · {num_turns} {turn_word}{tokens_str}");
        let hint = if self.messages.is_empty() {
            ""
        } else {
            "  (:N to view)"
        };

        queue!(self.out, Print("\r\n")).ok();
        queue!(
            self.out,
            Print(theme::result_line().apply(label)),
            Print(theme::dim().apply(stats)),
            Print(theme::dim().apply(hint)),
            Print("\r\n"),
        )
        .ok();
        self.out.flush().ok();
    }

    // --- Stream events ---

    pub fn handle_stream_event(&mut self, se: &StreamEvent) {
        match se.event.event_type.as_str() {
            "content_block_start" => {
                if let Some(ref cb) = se.event.content_block {
                    match cb.r#type.as_str() {
                        "text" => {
                            self.finish_current_block();
                            self.current_block = Some(BlockKind::Text);
                            self.text_streaming = false;
                        }
                        "tool_use" => {
                            self.finish_current_block();
                            let name = cb.name.as_deref().unwrap_or("unknown").to_string();
                            self.current_block = Some(BlockKind::ToolUse);
                            self.current_tool = Some((name, Value::Null));
                        }
                        "thinking" => {
                            self.finish_current_block();
                            self.current_block = Some(BlockKind::Thinking);
                            self.current_thinking = Some(String::new());
                            self.tool_counter += 1;
                            let n = self.tool_counter;
                            let label = format!("[{n}] Thinking...");
                            queue!(
                                self.out,
                                Print(theme::dim_italic().apply(label)),
                                Print("\r\n"),
                            )
                            .ok();
                            self.out.flush().ok();
                        }
                        _ => {}
                    }
                }
            }
            "content_block_delta" => {
                if let Some(ref delta) = se.event.delta {
                    match delta.r#type.as_str() {
                        "text_delta" => {
                            if let Some(ref text) = delta.text {
                                self.stream_text(text);
                            }
                        }
                        "input_json_delta" => {
                            // Accumulate tool input JSON — we'll render it on block_stop
                            if let Some(ref partial) = delta.partial_json
                                && let Some((_, input)) = &mut self.current_tool
                            {
                                if *input == Value::Null {
                                    *input = Value::String(partial.clone());
                                } else if let Value::String(s) = input {
                                    s.push_str(partial);
                                }
                            }
                        }
                        "thinking_delta" => {
                            if let Some(ref text) = delta.thinking
                                && let Some(ref mut buf) = self.current_thinking
                            {
                                buf.push_str(text);
                                if self.config.show_thinking {
                                    let text = text.replace('\n', "\r\n");
                                    queue!(self.out, Print(theme::dim_italic().apply(&text)),).ok();
                                    self.out.flush().ok();
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "content_block_stop" => {
                self.finish_current_block();
            }
            _ => {}
        }
    }

    // --- Tool results ---

    pub fn render_tool_result(&mut self, result: &Value, message: Option<&Value>) {
        let mut is_error = result
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || matches!(result, Value::String(s) if s.starts_with("Error"));

        // Also check message.content for is_error (the tool_use_result value
        // often lacks this field — it lives in the inner tool_result block).
        let msg_content_block = message
            .and_then(|m| m.get("content"))
            .and_then(Value::as_array)
            .and_then(|arr| arr.first());
        if !is_error && let Some(block) = msg_content_block {
            is_error = block
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        }

        // Extract text from tool_use_result, falling back to message.content
        let mut text = extract_result_text(result);
        if text.is_empty()
            && let Some(block) = msg_content_block
        {
            text = extract_result_text(block);
        }

        // Store result text on the most recent tool message
        if !text.is_empty()
            && let Some(msg) = self.messages.last_mut()
        {
            msg.result = Some(text.clone());
        }

        if is_error {
            self.render_error_line(&text);
        } else {
            self.close_tool_line();
        }
        self.out.flush().ok();
    }

    // --- Subagent tool calls (indented) ---

    pub fn render_subagent_tool_call(&mut self, name: &str, input: &Value) {
        self.close_tool_line();
        self.finish_current_block();
        self.render_tool_call_line(name, input, true);
        self.out.flush().ok();
    }

    pub fn render_subagent_tool_result(&mut self, message: &Value) {
        let Some(content) = message.get("content").and_then(Value::as_array) else {
            return;
        };
        for item in content {
            if item.get("type").and_then(Value::as_str) != Some("tool_result") {
                continue;
            }

            // Store result text on the most recent tool message
            let text = extract_result_text(item);
            if !text.is_empty()
                && let Some(msg) = self.messages.last_mut()
            {
                msg.result = Some(text.clone());
            }

            let is_error = item
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if is_error {
                self.render_error_line(&text);
            } else {
                self.close_tool_line();
            }
        }
        self.out.flush().ok();
    }

    // --- Prompt ---

    pub fn show_prompt(&mut self) {
        queue!(self.out, Print(theme::prompt_style().apply("> ")),).ok();
        self.out.flush().ok();
    }

    /// Ensure we're on a fresh line and show the `> ` input prompt prefix.
    /// Called when the user starts typing mid-stream.
    pub fn begin_input_line(&mut self) {
        self.close_tool_line();
        if self.text_streaming {
            queue!(self.out, Print("\r\n")).ok();
            self.text_streaming = false;
        }
        queue!(self.out, Print(theme::prompt_style().apply("> "))).ok();
        self.out.flush().ok();
    }

    /// Print a styled record of the user's message (e.g. `> hello`).
    pub fn render_user_message(&mut self, text: &str) {
        self.close_tool_line();
        if self.text_streaming {
            queue!(self.out, Print("\r\n")).ok();
            self.text_streaming = false;
        }
        let line = format!("> {text}");
        queue!(
            self.out,
            Print(theme::prompt_style().apply(line)),
            Print("\r\n"),
        )
        .ok();
        self.out.flush().ok();
    }

    // --- Internal ---

    /// Render a tool call line: `[N] ▶ ToolName  detail`. Subagent calls are
    /// indented and use a dimmer style. Leaves the line open for the result.
    fn render_tool_call_line(&mut self, name: &str, input: &Value, is_subagent: bool) {
        self.tool_counter += 1;
        self.last_tool_is_subagent = is_subagent;
        let n = self.tool_counter;
        let display_name = display_tool_name(name);
        let detail = format_tool_detail(name, input);

        let prefix = if is_subagent { "  " } else { "" };
        let label = truncate_line(&format!("{prefix}[{n}] ▶ {display_name}  {detail}"));
        let style = if is_subagent {
            theme::tool_name_dim()
        } else {
            theme::tool_name()
        };
        queue!(self.out, Print(style.apply(&label))).ok();

        let content = serde_json::to_string_pretty(input).unwrap_or_default();
        self.messages.push(StoredMessage {
            label: format!("[{n}] {display_name}"),
            content,
            result: None,
        });

        self.tool_line_open = true;
    }

    /// Render an error line beneath a tool call: `✗ <first line of error text>`.
    fn render_error_line(&mut self, text: &str) {
        self.close_tool_line();
        let indent = self.tool_indent();
        let error_line = if text.is_empty() {
            format!("{indent}✗")
        } else {
            let brief = first_line(text);
            format!("{indent}✗ {brief}")
        };
        let error_line = truncate_line(&error_line);
        queue!(
            self.out,
            Print(theme::error().apply(&error_line)),
            Print("\r\n"),
        )
        .ok();
    }

    /// Compute the indent string that aligns content under a `[N] ▶` prefix.
    /// For subagent calls, includes the extra 2-space indent.
    fn tool_indent(&self) -> String {
        // Width of "[N] " is: 1 + digit_count + 2
        let digits = digit_count(self.tool_counter);
        let base = digits + 3; // "[" + digits + "] "
        let extra = if self.last_tool_is_subagent { 2 } else { 0 };
        " ".repeat(base + extra)
    }

    /// Close an open tool call line if one is pending.
    fn close_tool_line(&mut self) {
        if self.tool_line_open {
            queue!(self.out, Print("\r\n")).ok();
            self.tool_line_open = false;
        }
    }

    fn stream_text(&mut self, text: &str) {
        let text = if self.text_streaming {
            text
        } else {
            // Strip leading newlines from the first delta in a text block.
            // Claude's API often prefixes responses with \n\n which creates
            // unwanted blank lines.
            let trimmed = text.trim_start_matches('\n');
            if trimmed.is_empty() {
                return;
            }
            self.text_streaming = true;
            trimmed
        };
        if text.is_empty() {
            return;
        }
        // Replace \n with \r\n for raw mode
        let text = text.replace('\n', "\r\n");
        queue!(self.out, Print(&text)).ok();
        self.out.flush().ok();
    }

    fn finish_current_block(&mut self) {
        match self.current_block.take() {
            Some(BlockKind::Text) => {
                self.close_tool_line();
                if self.text_streaming {
                    queue!(self.out, Print("\r\n\r\n")).ok();
                    self.text_streaming = false;
                }
            }
            Some(BlockKind::ToolUse) => {
                self.close_tool_line();
                if let Some((name, raw_input)) = self.current_tool.take() {
                    // Parse accumulated JSON
                    let input = match raw_input {
                        Value::String(s) => {
                            serde_json::from_str::<Value>(&s).unwrap_or(Value::Null)
                        }
                        other => other,
                    };
                    self.render_tool_call_line(&name, &input, false);
                }
            }
            Some(BlockKind::Thinking) => {
                self.close_tool_line();
                let content = self.current_thinking.take().unwrap_or_default();
                let n = self.tool_counter;
                if self.config.show_thinking && !content.is_empty() {
                    queue!(self.out, Print("\r\n\r\n")).ok();
                }
                self.messages.push(StoredMessage {
                    label: format!("[{n}] Thinking"),
                    content,
                    result: None,
                });
            }
            None => {}
        }
        self.out.flush().ok();
    }

    pub fn render_warning(&mut self, warning: &str) {
        queue!(
            self.out,
            Print(theme::dim().apply(format!("[warn] {warning}"))),
            Print("\r\n"),
        )
        .ok();
        self.out.flush().ok();
    }

    pub fn render_interrupted(&mut self) {
        self.close_tool_line();
        self.finish_current_block();
        queue!(
            self.out,
            Print("\r\n"),
            Print(theme::dim().apply("[interrupted]")),
            Print("\r\n"),
        )
        .ok();
        self.out.flush().ok();
    }

    pub fn render_exit(&mut self, code: Option<i32>) {
        let msg = match code {
            Some(c) => format!("Claude process exited with code {c}"),
            None => "Claude process exited".to_string(),
        };
        queue!(self.out, Print(theme::dim().apply(msg)), Print("\r\n"),).ok();
        self.out.flush().ok();
    }

    /// Write raw text (for input echo, etc.) with \r\n.
    pub fn write_raw(&mut self, text: &str) {
        queue!(self.out, Print(text)).ok();
        self.out.flush().ok();
    }
}

/// Format tool detail based on tool name and input.
fn format_tool_detail(name: &str, input: &Value) -> String {
    match name {
        "Read" => get_str(input, "file_path").unwrap_or_default().to_string(),
        "Edit" => {
            let path = get_str(input, "file_path").unwrap_or_default();
            let old_lines = get_str(input, "old_string").map_or(0, |s| s.lines().count());
            let new_lines = get_str(input, "new_string").map_or(0, |s| s.lines().count());
            let added = new_lines.saturating_sub(old_lines);
            let removed = old_lines.saturating_sub(new_lines);
            if added > 0 || removed > 0 {
                let diff = if added > 0 {
                    format!("+{added}")
                } else {
                    format!("-{removed}")
                };
                format!("({diff})  {path}")
            } else {
                path.to_string()
            }
        }
        "Write" => {
            let path = get_str(input, "file_path").unwrap_or_default();
            let lines = get_str(input, "content").map(|c| {
                let count = c.lines().count();
                if count == 1 {
                    "(1 line)".to_string()
                } else {
                    format!("({count} lines)")
                }
            });
            match lines {
                Some(l) => format!("{l}  {path}"),
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
            first_line(cmd).to_string()
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

fn digit_count(n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    let mut count = 0;
    let mut v = n;
    while v > 0 {
        count += 1;
        v /= 10;
    }
    count
}

/// Extract the first line of a string (no truncation).
fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

/// Truncate a string to fit within `max_width` display columns, appending `...` if truncated.
fn truncate_to_width(s: &str, max_width: usize) -> String {
    let ellipsis_width = 3; // "..."
    let mut width = 0;
    // Track the byte position where we'd cut for ellipsis
    let mut cut_pos = 0;
    let mut result = String::new();
    for ch in s.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width > max_width {
            // Need to truncate — use the cut point we've been tracking
            if max_width >= ellipsis_width {
                result.truncate(cut_pos);
                result.push_str("...");
            } else {
                result.clear();
            }
            return result;
        }
        result.push(ch);
        width += ch_width;
        // Track the latest position that leaves room for "..."
        if width <= max_width.saturating_sub(ellipsis_width) {
            cut_pos = result.len();
        }
    }
    result
}

/// Truncate a line to the current terminal width.
fn truncate_line(line: &str) -> String {
    truncate_to_width(line, term_width())
}

/// Query the current terminal width, defaulting to 80.
fn term_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
}

/// Shorten MCP tool names from `mcp__<server-key>__<tool>` to a colon-separated form.
///
/// Plugin keys encode `plugin:<id>:<name>` as `plugin_<id>_<name>`, so we decode
/// the first `_` back to `:` and strip the `plugin_` prefix. Non-plugin keys are
/// used as-is. Examples:
/// - `mcp__plugin_llms-fetch-mcp_llms-fetch__fetch` → `llms-fetch-mcp:llms-fetch:fetch`
/// - `mcp__my-server__do_thing` → `my-server:do_thing`
/// - `Read` → `Read` (non-MCP, unchanged)
fn display_tool_name(name: &str) -> String {
    let parts: Vec<&str> = name.splitn(3, "__").collect();
    if parts.len() == 3 && parts[0] == "mcp" {
        let server_key = parts[1];
        let tool = parts[2];
        if let Some(rest) = server_key.strip_prefix("plugin_") {
            // Plugin keys encode `:` as `_`: plugin_X_Y → X:Y
            let server = rest.replacen('_', ":", 1);
            format!("{server}:{tool}")
        } else {
            format!("{server_key}:{tool}")
        }
    } else {
        name.to_string()
    }
}

/// Extract displayable text from a tool result value.
/// Handles: objects with "content" (regular tools), arrays of content blocks (MCP tools),
/// and plain strings (permission errors).
fn extract_result_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Object(_) => value
            .get("content")
            .map(extract_result_text)
            .unwrap_or_default(),
        Value::Array(arr) => {
            for item in arr {
                if item.get("type").and_then(Value::as_str) == Some("text")
                    && let Some(text) = item.get("text").and_then(Value::as_str)
                {
                    return text.to_string();
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_tool_name_plugin_mcp() {
        assert_eq!(
            display_tool_name("mcp__plugin_llms-fetch-mcp_llms-fetch__fetch"),
            "llms-fetch-mcp:llms-fetch:fetch"
        );
    }

    #[test]
    fn display_tool_name_non_plugin_mcp() {
        assert_eq!(
            display_tool_name("mcp__my-server__do_thing"),
            "my-server:do_thing"
        );
    }

    #[test]
    fn display_tool_name_non_mcp() {
        assert_eq!(display_tool_name("Read"), "Read");
        assert_eq!(display_tool_name("Bash"), "Bash");
    }

    #[test]
    fn display_tool_name_not_enough_parts() {
        assert_eq!(display_tool_name("mcp__solo"), "mcp__solo");
    }

    #[test]
    fn first_line_extracts_first() {
        assert_eq!(first_line("hello\nworld"), "hello");
        assert_eq!(first_line("single"), "single");
        assert_eq!(first_line(""), "");
    }

    #[test]
    fn truncate_to_width_no_truncation() {
        assert_eq!(truncate_to_width("hello", 10), "hello");
        assert_eq!(truncate_to_width("hello", 5), "hello");
    }

    #[test]
    fn truncate_to_width_exact_fit() {
        assert_eq!(truncate_to_width("12345", 5), "12345");
    }

    #[test]
    fn truncate_to_width_truncates_with_ellipsis() {
        assert_eq!(truncate_to_width("hello world", 8), "hello...");
        assert_eq!(truncate_to_width("abcdefghij", 6), "abc...");
    }

    #[test]
    fn truncate_to_width_very_small_max() {
        // max_width < 3 can't even fit "..."
        assert_eq!(truncate_to_width("hello", 2), "");
        assert_eq!(truncate_to_width("hello", 3), "...");
    }

    #[test]
    fn truncate_to_width_empty_string() {
        assert_eq!(truncate_to_width("", 10), "");
    }

    #[test]
    fn truncate_to_width_wide_chars() {
        // CJK characters are 2 display columns wide
        // "漢字" is 4 columns, "ab" is 2 columns = 6 total
        assert_eq!(truncate_to_width("漢字ab", 10), "漢字ab");
        // Truncate: 6 cols needed, max 5 → need ellipsis, target=2, "漢" is 2 cols
        assert_eq!(truncate_to_width("漢字ab", 5), "漢...");
    }

    #[test]
    fn format_tool_detail_read() {
        let input = serde_json::json!({"file_path": "/src/main.rs"});
        assert_eq!(format_tool_detail("Read", &input), "/src/main.rs");
    }

    #[test]
    fn format_tool_detail_edit_with_additions() {
        let input = serde_json::json!({
            "file_path": "/src/main.rs",
            "old_string": "line1",
            "new_string": "line1\nline2\nline3"
        });
        assert_eq!(format_tool_detail("Edit", &input), "(+2)  /src/main.rs");
    }

    #[test]
    fn format_tool_detail_edit_with_removals() {
        let input = serde_json::json!({
            "file_path": "/src/main.rs",
            "old_string": "line1\nline2\nline3",
            "new_string": "line1"
        });
        assert_eq!(format_tool_detail("Edit", &input), "(-2)  /src/main.rs");
    }

    #[test]
    fn format_tool_detail_edit_net_additions() {
        let input = serde_json::json!({
            "file_path": "/src/main.rs",
            "old_string": "aaa\nbbb\nccc",
            "new_string": "xxx\nyyy\nzzz\nwww\nvvv"
        });
        assert_eq!(format_tool_detail("Edit", &input), "(+2)  /src/main.rs");
    }

    #[test]
    fn format_tool_detail_edit_same_line_count() {
        let input = serde_json::json!({
            "file_path": "/src/main.rs",
            "old_string": "old_value",
            "new_string": "new_value"
        });
        // Same line count → no diff stats
        assert_eq!(format_tool_detail("Edit", &input), "/src/main.rs");
    }

    #[test]
    fn format_tool_detail_write_single_line() {
        let input = serde_json::json!({
            "file_path": "/hello.txt",
            "content": "Hello, world!"
        });
        assert_eq!(format_tool_detail("Write", &input), "(1 line)  /hello.txt");
    }

    #[test]
    fn format_tool_detail_write_multiple_lines() {
        let input = serde_json::json!({
            "file_path": "/hello.py",
            "content": "print('hello')\nprint('world')\n"
        });
        assert_eq!(format_tool_detail("Write", &input), "(2 lines)  /hello.py");
    }

    #[test]
    fn format_tool_detail_write_trailing_newline() {
        // str::lines() doesn't count a trailing newline as an extra line
        let input = serde_json::json!({
            "file_path": "/hello.txt",
            "content": "single line\n"
        });
        assert_eq!(format_tool_detail("Write", &input), "(1 line)  /hello.txt");
    }

    #[test]
    fn format_tool_detail_write_no_content() {
        let input = serde_json::json!({"file_path": "/empty.txt"});
        assert_eq!(format_tool_detail("Write", &input), "/empty.txt");
    }

    #[test]
    fn format_tool_detail_glob() {
        let input = serde_json::json!({"pattern": "**/*.rs"});
        assert_eq!(format_tool_detail("Glob", &input), "**/*.rs");
    }

    #[test]
    fn format_tool_detail_grep_with_path() {
        let input = serde_json::json!({"pattern": "fn main", "path": "/src"});
        assert_eq!(format_tool_detail("Grep", &input), "fn main  /src");
    }

    #[test]
    fn format_tool_detail_grep_without_path() {
        let input = serde_json::json!({"pattern": "TODO"});
        assert_eq!(format_tool_detail("Grep", &input), "TODO");
    }

    #[test]
    fn format_tool_detail_bash() {
        let input = serde_json::json!({"command": "ls -la\necho done"});
        assert_eq!(format_tool_detail("Bash", &input), "ls -la");
    }

    #[test]
    fn format_tool_detail_task() {
        let input = serde_json::json!({"description": "Summarize README"});
        assert_eq!(format_tool_detail("Task", &input), "Summarize README");
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
