---
priority: P2
state: new
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
