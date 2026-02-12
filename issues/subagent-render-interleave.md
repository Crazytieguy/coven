---
priority: P2
state: approved
---

# Subagent tool call rendering interleaves across lines

When multiple subagents are active, tool call lines sometimes get merged onto the same line instead of each printing on its own line:

```
[17] ▶ Task  Explore VCR test structure
[18] ▶ Task    [17/1] ▶ Bash  find /Users/yoav/.coven/worktrees/coven/cool-stream-28/tests -type f -name "*.rs" -o -type f -name "*.md" -o -type d | hea...
```

Expected: each line should be printed in full on its own line. The `[17/1] ▶ Bash ...` line should start on a new line, not appended to `[18] ▶ Task`.

## Plan

### Root cause

In `src/display/renderer.rs`, `render_tool_call_line()` (line 557) queues a tool call label without first closing any previously-open tool line. Normally this is fine because `finish_current_block()` calls `close_tool_line()` before calling `render_tool_call_line()`. But there's a gap in `render_subagent_tool_call()` (line 361):

```rust
pub fn render_subagent_tool_call(...) {
    self.finish_current_block();          // may OPEN a new tool line
    self.render_tool_call_line(...);      // appends to whatever line is current
    self.out.flush().ok();
}
```

`finish_current_block()` first closes any open tool line, but then if there's a pending `ToolUse` block (from the main stream still in progress), it renders that block via `render_tool_call_line()`, setting `tool_line_open = true` again. The subsequent `render_tool_call_line()` call for the subagent's tool then prints on the same line.

This happens when the Claude CLI interleaves subagent events with the main assistant message stream — e.g., a subagent event for tool [17] arrives while the main stream is still mid-block for tool [18].

### Fix

Add `self.close_tool_line()` at the beginning of `render_tool_call_line()`:

```rust
fn render_tool_call_line(
    &mut self,
    name: &str,
    input: &Value,
    parent_tool_use_id: Option<&str>,
) {
    self.close_tool_line();  // <-- add this line
    let display_name = display_tool_name(name);
    // ... rest unchanged
}
```

`close_tool_line()` is idempotent (no-op when `tool_line_open` is false), so existing call paths through `finish_current_block()` are unaffected — the prior `close_tool_line()` already set `tool_line_open = false`.

### Test

Add a unit test in the `renderer.rs` `#[cfg(test)]` module that constructs a `Renderer<Vec<u8>>` and simulates the interleaving scenario:

1. Set up a renderer with an in-progress ToolUse block (`current_block = Some(ToolUse)`, `current_tool` set, with a registered active subagent)
2. Leave `tool_line_open = true` (simulating a previously-rendered tool line)
3. Call `render_subagent_tool_call()`
4. Assert the output contains `\r\n` between each tool call line (no concatenation)

Since the renderer fields are private, the test can set up the state through the public API: call `handle_stream_event()` with synthetic `content_block_start`/`content_block_delta`/`content_block_stop` events to build up the right state, then call `render_subagent_tool_call()` and check the output.

### Files to modify

- `src/display/renderer.rs`: Add `self.close_tool_line()` at the top of `render_tool_call_line()`, add unit test

Review note: While you're at it, make sure there are no other race conditions in the rendering code.
