---
priority: P2
state: new
---

# `InputHandler` editing behavior has no unit tests

`src/display/input.rs` contains significant editing logic — word boundary navigation, cursor movement, character insertion/deletion, Ctrl+U/K/W shortcuts, word-based deletion — but only `parse_view_command` has unit tests.

The `InputHandler` struct is tested indirectly through VCR integration tests (via trigger messages), but these only exercise the submit/follow-up/interrupt code paths. Fine-grained editing behavior (word boundaries, cursor positioning, delete-range operations) is untested.

**Specific untested behaviors:**
- `word_boundary_left` / `word_boundary_right` (lines 78-106) — boundary detection with multi-word buffers, leading/trailing whitespace, single-character words
- `delete_range` with word boundaries (Ctrl+W, Alt+Backspace, Alt+D)
- `byte_pos_at` with multi-byte Unicode characters
- `handle_key` edge cases: Backspace at position 0, Delete at end, Home/End keys, empty buffer submit

**Approach:** These functions are testable in isolation without terminal state since the cursor/buffer logic is separate from the `redraw()` rendering. Tests can create an `InputHandler`, call `handle_key` with synthetic `KeyEvent`s, and assert on the buffer/cursor state.
