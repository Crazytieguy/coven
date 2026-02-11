Issue: [P1] :N view for common claude code tools should display in a nicer format than just the raw json
Status: draft

## Approach

Currently `format_message()` in `src/display/renderer.rs` shows tool inputs as `serde_json::to_string_pretty()` and results as plain text separated by `--- Result ---`. This is functional but hard to scan, especially for tools with large inputs like Edit or Write.

Add a `format_tool_view(name: &str, input: &Value, result: Option<&str>) -> String` function that dispatches on tool name and produces human-friendly output. Fall back to the current pretty-JSON for unknown tools.

### Tool-specific formatters

| Tool | Input display | Result display |
|------|--------------|----------------|
| **Read** | `file_path` as a header, with offset/limit if present | Unchanged (already has line numbers) |
| **Edit** | `file_path` header, then a unified-diff-style view: lines prefixed with `-` (old_string) and `+` (new_string), styled red/green | Unchanged (usually brief confirmation) |
| **Write** | `file_path` header, then content with line numbers | Unchanged |
| **Bash** | `$ command` rendered as a shell prompt line (multi-line preserved) | Unchanged (already reads like terminal output) |
| **Glob** | `pattern` as header, `path` if present | Unchanged |
| **Grep** | `pattern` + flags summary, `path` if present | Unchanged |
| **WebFetch** | URL as header, prompt text below | Unchanged |
| **WebSearch** | Query as header | Unchanged |
| **Task** | Description as header, subagent_type if present | Unchanged |

### Where to put it

New function `format_tool_view()` in `src/display/renderer.rs` (near existing `format_message()`). It takes the tool name, parsed JSON `Value` of the input, and optional result text, returning a styled `String`. The existing `format_message()` calls this instead of `to_string_pretty()`.

To parse the input back from the stored pretty-printed JSON string, use `serde_json::from_str`. Store the original `Value` in `StoredMessage` instead of the stringified JSON to avoid the round-trip — change `content: String` to `input: Value` and derive the display from that.

### Styling

Use the existing `theme.rs` ANSI styles. For the diff view in Edit:
- Red (`\x1b[31m`) for removed lines (old_string)
- Green (`\x1b[32m`) for added lines (new_string)
- These are standard ANSI colors compatible with the project's named-color convention.

### Changes summary

1. `StoredMessage`: change `content: String` to `input: serde_json::Value` (or add `input_json: Value` alongside)
2. `format_message()`: call `format_tool_view()` instead of returning pretty JSON directly
3. New `format_tool_view()`: match on tool name, produce formatted output, fall back to pretty JSON
4. Adjust `finish_current_block()` where `StoredMessage` is created — store parsed Value

### Not in scope

- Syntax highlighting for code (would need a highlighting crate)
- Result formatting changes (results are already reasonably readable)
- Changes to the inline tool call line rendering (that's already good via `format_tool_detail()`)

## Questions

### Should we store Value instead of String in StoredMessage?

Storing the parsed `Value` avoids a round-trip through `to_string_pretty` → `from_str`. The downside is a slightly larger in-memory footprint (Value vs String), but messages are few and short-lived.

Option A: Store `Value`, derive display in `format_tool_view()`
Option B: Keep `String`, parse it back to `Value` in `format_tool_view()` (simpler change, tiny perf cost)

Leaning toward Option B since it's a smaller change and the parse cost is negligible.

Answer:

## Review

