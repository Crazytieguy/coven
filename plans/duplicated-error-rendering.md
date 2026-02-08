Issue: Duplicated error rendering: `render_tool_result` and `render_subagent_tool_result` in renderer.rs have identical error display logic (close_tool_line, indent, format `✗` line, truncate, print) — extract a shared helper
Status: draft

## Approach

Extract a private helper method on `Renderer` that encapsulates the shared error display logic:

```rust
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
```

Then replace the duplicated blocks in both `render_tool_result` (lines 282-297) and `render_subagent_tool_result` (lines 351-366) with calls to `self.render_error_line(&text)`.

In `render_tool_result`, the `else` branch still calls `self.close_tool_line()` — that stays as-is since the non-error path doesn't need the helper.

In `render_subagent_tool_result`, the non-error path also calls `self.close_tool_line()` (line 368) — same treatment.

**Files changed:** `src/display/renderer.rs` only.

## Review

