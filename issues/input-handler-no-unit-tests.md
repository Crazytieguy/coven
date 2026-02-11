---
priority: P2
state: new
---

# InputHandler editing logic has no unit tests

`src/display/input.rs` implements a line editor (`InputHandler`) with complex editing behavior:

- Character insertion at cursor position (`insert_char`)
- Cursor movement: left/right, Home/End, Ctrl+A/E (`move_cursor`)
- Word boundary navigation: Ctrl/Alt+Left/Right, Alt+B/F (`word_boundary_left`, `word_boundary_right`)
- Deletion: Backspace, Delete, Ctrl+W (word delete), Ctrl+U (line-to-start), Ctrl+K (line-to-end), Alt+D (word forward), Alt+Backspace (word backward)
- Range deletion (`delete_range`)
- Submit handling with steering vs follow-up mode detection (`handle_enter`)
- Activate/deactivate lifecycle

Only the `parse_view_command` helper function has unit tests. The core editing behavior — character insertion, cursor movement, word boundaries, deletion — is entirely untested.

## Impact

Editing regressions (e.g., off-by-one in cursor positioning, incorrect word boundary detection with unicode) would go undetected. The `byte_pos_at` helper that converts char indices to byte offsets is particularly sensitive to correctness.

## Note

The existing issue `format-message-no-unit-tests` covers `format_message`/`resolve_query`/`format_tool_view` in `renderer.rs`. This issue covers the separate `InputHandler` module.
