---
priority: P2
state: review
---

# No unit tests for message resolution and tool view formatting

`src/display/renderer.rs:29-77` contains `format_message`, `resolve_query`, and `format_tool_view` — functions that handle the `:N` message viewing feature. These have no direct unit tests, only indirect coverage via VCR integration tests.

## Untested logic

- `resolve_query` (line 49-77): Resolves numeric (`"3"`), parent/child (`"2/1"`), and label-based queries (`"Bash"`, `"Edit[-1]"`) against stored messages. Includes negative indexing for label queries.
- `format_tool_view` (line 769-841): Tool-specific formatting for 10+ tool types (Read, Edit, Write, Bash, Glob, Grep, WebFetch, WebSearch, Task, etc.).
- `format_message` (line 29-46): Composes the final view by combining label, formatted content, and optional result text.

## Suggested test cases

- `resolve_query` with numeric, P/C, and label-based lookups
- `resolve_query` with negative index (`Edit[-1]`)
- `resolve_query` with no match
- `format_tool_view` for Edit (shows diff), Write (shows line numbers), Bash (shows command)
- `format_message` with and without result text

## Plan

Add tests to the existing `#[cfg(test)] mod tests` block at `src/display/renderer.rs:1014`. Follow the existing style: simple arrange-act-assert with `serde_json::json!` for inputs.

### 1. Add a `make_message` helper

```rust
fn make_message(label: &str, content: &str, result: Option<&str>) -> StoredMessage {
    StoredMessage {
        label: label.to_string(),
        content: content.to_string(),
        result: result.map(str::to_string),
    }
}
```

### 2. `resolve_query` tests

The function is private, but it's accessible from the `tests` module via `use super::*`. Test cases:

- **Numeric query**: `"3"` finds message with label `"[3] Bash"` among several messages
- **Parent/child query**: `"2/1"` finds message with label `"[2/1] Read"`
- **Numeric no match**: `"99"` returns `None`
- **Label default index**: `"Bash"` returns the first message whose tool name is `Bash` (index 0)
- **Label explicit index**: `"Read[1]"` returns the second `Read` message
- **Label negative index**: `"Edit[-1]"` returns the last `Edit` message
- **Label out of bounds**: `"Bash[5]"` returns `None` when fewer than 6 Bash messages exist
- **Label no match**: `"NotATool"` returns `None`
- **Empty name**: `"[0]"` (parses as empty name) returns `None`

Use a shared set of ~5 test messages covering `[1] Bash`, `[2] Read`, `[2/1] Read`, `[3] Edit`, `[4] Edit` to exercise all paths.

### 3. `format_tool_view` tests

Each test constructs a `serde_json::json!` input and checks the formatted output. Cover:

- **Read**: path only → just the path string
- **Read with offset+limit**: includes `(offset: N, limit: M)` suffix
- **Edit**: shows file path, red `- old` lines, green `+ new` lines (assert ANSI codes `\x1b[31m`/`\x1b[32m`)
- **Write**: shows file path followed by numbered lines (`   1  line`, `   2  line`)
- **Bash**: `$ command` format
- **Glob with path**: `pattern  in path`
- **Grep without path**: `/pattern/`
- **WebFetch with prompt**: `url\n\nprompt`
- **WebSearch**: just the query string
- **Task with subagent_type**: `[type] description`
- **Unknown tool**: returns `None`
- **Missing required field**: e.g. `Read` without `file_path` returns `None`

### 4. `format_message` tests

These compose `resolve_query` + `format_tool_view` + result formatting:

- **With tool-specific formatting, no result**: message with valid JSON `Bash` input → output includes label, `$ command`, no result separator
- **With result**: same but with `result: Some("output")` → output includes `--- Result ---\noutput`
- **Fallback to raw content**: message with non-JSON content → output includes label and raw content string
- **Unknown tool JSON**: message labeled `[1] CustomTool` with JSON content → falls back to raw JSON since `format_tool_view` returns `None`
- **No match**: query `"99"` → returns `None`

### 5. `tool_name_from_label` tests (bonus, tiny)

- `"[1] Bash"` → `"Bash"`
- `"[2/1] Read"` → `"Read"`
- `"NoBracket"` → `"NoBracket"`

### Summary

~25 tests total, all added to the existing `mod tests` block. No new files, no new dependencies, no mocking.
