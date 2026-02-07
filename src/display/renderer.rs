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
            out: writer,
        }
    }

    pub fn messages(&self) -> &[StoredMessage] {
        &self.messages
    }

    // --- Session lifecycle ---

    pub fn render_session_header(&mut self, session_id: &str, model: &str) {
        let header = format!("Session {session_id} ({model})");
        queue!(self.out, Print(theme::dim().apply(header)), Print("\r\n")).ok();
        queue!(self.out, Print("\r\n")).ok();
        self.out.flush().ok();
    }

    pub fn render_result(&mut self, subtype: &str, cost: f64, duration_ms: u64, num_turns: u32) {
        self.finish_current_block();
        let secs = duration_ms as f64 / 1000.0;

        let label = if subtype == "success" {
            "Done"
        } else {
            "Error"
        };
        let stats = format!("  ${cost:.2} · {secs:.1}s · {num_turns} turns");
        let hint = if !self.messages.is_empty() {
            "  (:N to view)"
        } else {
            ""
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
                            queue!(
                                self.out,
                                Print(theme::dim_italic().apply("Thinking...")),
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
                            // Thinking content hidden — just show "Thinking..."
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

    pub fn render_tool_result(&mut self, result: &Value) {
        let is_error = result
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || matches!(result, Value::String(s) if s.starts_with("Error"));

        if is_error {
            queue!(self.out, Print("    "), Print(theme::error().apply("✗")),).ok();
            let text = extract_result_text(result);
            if !text.is_empty() {
                let brief = first_line_truncated(&text, 60);
                queue!(self.out, Print(" "), Print(theme::error().apply(brief)),).ok();
            }
            queue!(self.out, Print("\r\n")).ok();
        } else {
            queue!(self.out, Print("    "), Print(theme::success().apply("✓")),).ok();
            let text = extract_result_text(result);
            if !text.is_empty() {
                let brief = first_line_truncated(&text, 60);
                if !brief.is_empty() {
                    queue!(self.out, Print(" "), Print(theme::dim().apply(brief)),).ok();
                }
            }
            queue!(self.out, Print("\r\n")).ok();
        }
        self.out.flush().ok();
    }

    // --- Prompt ---

    pub fn show_prompt(&mut self) {
        queue!(self.out, Print(theme::prompt_style().apply("> ")),).ok();
        self.out.flush().ok();
    }

    // --- Internal ---

    fn stream_text(&mut self, text: &str) {
        if !self.text_streaming {
            self.text_streaming = true;
        }
        // Replace \n with \r\n for raw mode
        let text = text.replace('\n', "\r\n");
        queue!(self.out, Print(&text)).ok();
        self.out.flush().ok();
    }

    fn finish_current_block(&mut self) {
        match self.current_block.take() {
            Some(BlockKind::Text) => {
                if self.text_streaming {
                    queue!(self.out, Print("\r\n\r\n")).ok();
                    self.text_streaming = false;
                }
            }
            Some(BlockKind::ToolUse) => {
                if let Some((name, raw_input)) = self.current_tool.take() {
                    self.tool_counter += 1;
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
                    queue!(
                        self.out,
                        Print(theme::tool_name().apply(&label)),
                        Print("\r\n"),
                    )
                    .ok();

                    // Store for :N viewing
                    let content = serde_json::to_string_pretty(&input).unwrap_or_default();
                    self.messages.push(StoredMessage {
                        label: format!("[{n}] {name}"),
                        content,
                    });
                }
            }
            Some(BlockKind::Thinking) | None => {}
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
        "Write" => {
            let path = get_str(input, "file_path").unwrap_or_default();
            let lines = get_str(input, "content")
                .map(|c| format!("({} lines)", c.lines().count()))
                .unwrap_or_default();
            format!("{path} {lines}").trim().to_string()
        }
        "Edit" => get_str(input, "file_path").unwrap_or_default().to_string(),
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
fn extract_result_text(result: &Value) -> String {
    match result {
        Value::String(s) => s.clone(),
        Value::Object(_) => {
            if let Some(content) = result.get("content") {
                extract_content_text(content)
            } else {
                String::new()
            }
        }
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

fn extract_content_text(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
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
