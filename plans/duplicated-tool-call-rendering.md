Issue: Duplicated tool call rendering: `finish_current_block` (ToolUse case) and `render_subagent_tool_call` in renderer.rs have nearly identical logic — extract a shared helper parameterized by `is_subagent`
Status: draft

## Approach

Extract a private helper method on `Renderer` that encapsulates the shared tool-call rendering logic:

```rust
fn render_tool_call_line(&mut self, name: &str, input: &Value, is_subagent: bool) {
    self.tool_counter += 1;
    self.last_tool_is_subagent = is_subagent;
    let n = self.tool_counter;
    let display_name = display_tool_name(name);
    let detail = format_tool_detail(name, input);

    let prefix = if is_subagent { "  " } else { "" };
    let label = truncate_line(&format!("{prefix}[{n}] ▶ {display_name}  {detail}"));
    let style = if is_subagent { theme::tool_name_dim() } else { theme::tool_name() };
    queue!(self.out, Print(style.apply(&label))).ok();

    let content = serde_json::to_string_pretty(input).unwrap_or_default();
    self.messages.push(StoredMessage {
        label: format!("[{n}] {display_name}"),
        content,
        result: None,
    });

    self.tool_line_open = true;
}
```

**Callers:**

1. `render_subagent_tool_call` — simplifies to `close_tool_line()`, `finish_current_block()`, then `self.render_tool_call_line(name, input, true)`, flush.

2. `finish_current_block` ToolUse arm — keeps the JSON string parsing of `raw_input`, then calls `self.render_tool_call_line(&name, &input, false)` instead of the inline block.

No public API changes. No test changes needed — behavior is identical.

## Review

