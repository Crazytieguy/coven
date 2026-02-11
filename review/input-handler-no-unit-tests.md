---
priority: P2
state: review
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

## Plan

All changes are in `src/display/input.rs`.

### 1. Add `#[cfg(test)]` getter methods

The `buffer` and `cursor` fields are private, so tests need accessors. Add to the `impl InputHandler` block:

```rust
#[cfg(test)]
fn buffer(&self) -> &str {
    &self.buffer
}

#[cfg(test)]
fn cursor_pos(&self) -> usize {
    self.cursor
}
```

These are private to the module (usable by the inline `#[cfg(test)] mod tests`) and compile-gated so they don't exist in release builds.

### 2. Test helper

Add a helper function in the `tests` module to reduce boilerplate:

```rust
fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}
```

Also a helper that types a string into an activated handler:

```rust
fn type_str(input: &mut InputHandler, s: &str) {
    for c in s.chars() {
        input.handle_key(&key(KeyCode::Char(c), KeyModifiers::NONE));
    }
}
```

### 3. Tests to add

All tests go in the existing `#[cfg(test)] mod tests` block in `input.rs`, alongside the `parse_view_command` tests. Each test creates `InputHandler::new(2)`, calls `activate()`, then exercises behavior via `handle_key()` with synthetic `KeyEvent`s.

The `redraw()` calls inside `handle_key` will harmlessly write escape codes to stdout (crossterm queue + `term_width()` falls back to 80 without a terminal).

**Word boundary navigation:**
- `word_boundary_left_basic` — type `"hello world"`, cursor at end, Ctrl+Left moves to index 6, again to 0
- `word_boundary_right_basic` — cursor at 0, Ctrl+Right moves to 6, again to 11
- `word_boundary_with_multiple_spaces` — `"a  b"`, test that boundary skips the double space
- `word_boundary_at_edges` — word_boundary_left at 0 stays at 0, word_boundary_right at end stays at end

**Character insertion and cursor movement:**
- `insert_moves_cursor` — type `"abc"`, buffer is `"abc"`, cursor is 3
- `left_right_movement` — type `"abc"`, Left, Left, cursor is 1; Right, cursor is 2
- `home_end_keys` — type `"abc"`, Home → cursor 0, End → cursor 3
- `ctrl_a_ctrl_e` — same as Home/End via Ctrl+A/Ctrl+E

**Deletion:**
- `backspace_deletes_before_cursor` — type `"abc"`, Backspace → buffer `"ab"`, cursor 2
- `backspace_at_start_is_noop` — cursor at 0, Backspace does nothing
- `delete_key` — type `"abc"`, Home, Delete → buffer `"bc"`, cursor 0
- `delete_at_end_is_noop` — cursor at end, Delete does nothing
- `ctrl_u_deletes_to_start` — type `"hello world"`, Ctrl+U → buffer empty, cursor 0
- `ctrl_k_deletes_to_end` — type `"hello world"`, Home, Ctrl+K → buffer empty, cursor 0; also: cursor in middle deletes only right portion
- `ctrl_w_deletes_word_back` — type `"hello world"`, Ctrl+W → buffer `"hello "`, cursor 6
- `alt_d_deletes_word_forward` — type `"hello world"`, Home, Alt+D → buffer `" world"`, cursor 0
- `alt_backspace_deletes_word_back` — same as Ctrl+W behavior

**Multi-byte Unicode:**
- `unicode_char_insertion` — type `"café"`, buffer is `"café"`, cursor is 4
- `unicode_backspace` — type `"café"`, Backspace → buffer `"caf"`, cursor 3
- `unicode_cursor_movement` — type `"café"`, Left, Left → cursor 2, buffer unchanged

**Submit / mode detection:**
- `enter_submits_as_steering` — type `"hello"`, Enter → `Submit("hello", InputMode::Steering)`
- `alt_enter_submits_as_followup` — type `"hello"`, Alt+Enter → `Submit("hello", InputMode::FollowUp)`
- `empty_submit_is_noop` — activate, Enter → `InputAction::None`
- `escape_cancels` — type `"hello"`, Esc → `InputAction::Cancel`, `is_active()` is false
- `view_command_via_handle_key` — type `":3"`, Enter → `ViewMessage("3")`

**Inactive mode:**
- `inactive_char_activates` — new handler (inactive), type `'x'` → `Activated('x')`, `is_active()` is true, buffer is `"x"`
- `inactive_ctrl_c` — `Interrupt`
- `inactive_ctrl_d` — `EndSession`
- `inactive_non_char_ignored` — Left arrow while inactive → `None`, still inactive
