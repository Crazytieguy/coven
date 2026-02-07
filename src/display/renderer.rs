use std::io::{self, Write};

use crossterm::queue;
use crossterm::style::Print;
use serde_json::Value;

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
        let stats =
            format!("  ${cost:.2} · {whole_secs}.{tenths}s · {num_turns} turns{tokens_str}");
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
            self.close_tool_line();
            let indent = self.tool_indent();
            queue!(self.out, Print(indent), Print(theme::error().apply("✗")),).ok();
            if !text.is_empty() {
                let brief = first_line_truncated(&text, 60);
                queue!(self.out, Print(" "), Print(theme::error().apply(brief)),).ok();
            }
            queue!(self.out, Print("\r\n")).ok();
        } else {
            self.close_tool_line();
        }
        self.out.flush().ok();
    }

    // --- Subagent tool calls (indented) ---

    pub fn render_subagent_tool_call(&mut self, name: &str, input: &Value) {
        self.close_tool_line();
        self.finish_current_block();
        self.tool_counter += 1;
        self.last_tool_is_subagent = true;
        let n = self.tool_counter;
        let detail = format_tool_detail(name, input);
        let label = format!("  [{n}] ▶ {name}  {detail}");
        queue!(self.out, Print(theme::tool_name_dim().apply(&label)),).ok();

        // Store for :N viewing
        let content = serde_json::to_string_pretty(input).unwrap_or_default();
        self.messages.push(StoredMessage {
            label: format!("[{n}] {name}"),
            content,
            result: None,
        });

        // Leave line open — subagent result will close or print ✗
        self.tool_line_open = true;
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
                self.close_tool_line();
                let indent = self.tool_indent();
                queue!(self.out, Print(indent), Print(theme::error().apply("✗")),).ok();
                if !text.is_empty() {
                    let brief = first_line_truncated(&text, 60);
                    queue!(self.out, Print(" "), Print(theme::error().apply(brief)),).ok();
                }
                queue!(self.out, Print("\r\n")).ok();
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

    // --- Internal ---

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
            self.text_streaming = true;
            // Strip leading newlines from the first delta in a text block.
            // Claude's API often prefixes responses with \n\n which creates
            // unwanted blank lines.
            text.trim_start_matches('\n')
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
                    self.tool_counter += 1;
                    self.last_tool_is_subagent = false;
                    let n = self.tool_counter;

                    // Parse accumulated JSON
                    let input = match raw_input {
                        Value::String(s) => {
                            serde_json::from_str::<Value>(&s).unwrap_or(Value::Null)
                        }
                        other => other,
                    };

                    let detail = format_tool_detail(&name, &input);
                    let label = format!("[{n}] ▶ {name}  {detail}");
                    queue!(self.out, Print(theme::tool_name().apply(&label)),).ok();

                    // Store for :N viewing
                    let content = serde_json::to_string_pretty(&input).unwrap_or_default();
                    self.messages.push(StoredMessage {
                        label: format!("[{n}] {name}"),
                        content,
                        result: None,
                    });

                    // Leave line open — result will close or print ✗
                    self.tool_line_open = true;
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
                let diff = match (added > 0, removed > 0) {
                    (true, true) => format!("+{added} -{removed}"),
                    (true, false) => format!("+{added}"),
                    (false, true) => format!("-{removed}"),
                    (false, false) => unreachable!(),
                };
                format!("{path} ({diff})")
            } else {
                path.to_string()
            }
        }
        "Write" => {
            let path = get_str(input, "file_path").unwrap_or_default();
            let lines = get_str(input, "content")
                .map(|c| format!("({} lines)", c.lines().count()))
                .unwrap_or_default();
            format!("{path} {lines}").trim().to_string()
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
            first_line_truncated(cmd, 60)
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
                        return first_line_truncated(s, 60);
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

fn first_line_truncated(s: &str, max: usize) -> String {
    let line = s.lines().next().unwrap_or("");
    if line.len() > max {
        format!("{}...", &line[..max])
    } else {
        line.to_string()
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
