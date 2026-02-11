---
priority: P2
state: review
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

## Dependency

**Depends on:** `review/input-handler-bypasses-renderer-writer.md` — the writer-threading issue must land first. Currently `insert_char`, `delete_range`, `move_cursor`, `handle_key`, and `handle_enter` all call `redraw()` / `clear_input_lines()` which hardcode `io::stdout()`. Once a `&mut impl Write` parameter is threaded through, tests can pass a `Vec<u8>` and test the full editing pipeline without terminal side effects.

The pure logic helpers (`byte_pos_at`, `word_boundary_left`, `word_boundary_right`) and state transitions (`activate`, `deactivate`) have no IO dependency and can be tested immediately, but for completeness the full test suite should be added in one pass after the writer threading lands.

## Plan

All tests go in the existing `#[cfg(test)] mod tests` block in `src/display/input.rs`. Since the test module is in the same file, it has access to private fields and methods.

### 1. Helper: construct an `InputHandler` with preset state

Add a test helper that creates an `InputHandler` with a given buffer and cursor position, in active state:

```rust
fn handler_with(text: &str, cursor: usize) -> InputHandler {
    let mut h = InputHandler::new(2);
    h.buffer = text.to_string();
    h.cursor = cursor;
    h.active = true;
    h
}
```

### 2. `byte_pos_at` tests

Test the char-index to byte-offset conversion:

- **ASCII:** `"hello"` — `byte_pos_at(0)` = 0, `byte_pos_at(3)` = 3, `byte_pos_at(5)` = 5
- **Multibyte:** `"café"` — `byte_pos_at(3)` = 3, `byte_pos_at(4)` = 5 (the `é` is 2 bytes)
- **CJK:** `"ab日本"` — `byte_pos_at(2)` = 2, `byte_pos_at(3)` = 5, `byte_pos_at(4)` = 8
- **Past end:** `byte_pos_at(999)` returns `buffer.len()`
- **Empty buffer:** `byte_pos_at(0)` = 0

### 3. `word_boundary_left` tests

Set up `handler_with(text, cursor)` and assert `word_boundary_left()` returns the expected char index:

- **Middle of word:** `"hello world"`, cursor=8 → 6
- **At word start:** `"hello world"`, cursor=6 → 0
- **Multiple spaces:** `"foo  bar"`, cursor=5 → 0
- **At start:** `"hello"`, cursor=0 → 0
- **End of buffer:** `"hello world"`, cursor=11 → 6
- **Only spaces left:** `"   hello"`, cursor=3 → 0

### 4. `word_boundary_right` tests

Same pattern:

- **Middle of word:** `"hello world"`, cursor=2 → 6
- **At space:** `"hello world"`, cursor=5 → 11
- **Multiple spaces:** `"foo  bar"`, cursor=3 → 8
- **At end:** `"hello"`, cursor=5 → 5
- **Start of buffer:** `"hello world"`, cursor=0 → 6

### 5. `activate` / `deactivate` state transition tests

- `new()` → `is_active()` is false
- `activate()` → `is_active()` is true, buffer is empty, cursor is 0
- After editing, `deactivate()` → `is_active()` is false, buffer is empty, cursor is 0
- `activate()` after `deactivate()` resets state cleanly

### 6. Editing method tests (requires writer threading)

After the writer-threading issue lands, all editing methods accept `out: &mut impl Write`. Tests pass `&mut Vec::<u8>::new()` (or `&mut io::sink()`) as the writer.

**`insert_char`:**
- Insert at start: `handler_with("ello", 0)` → `insert_char('h', &mut sink())` → buffer `"hello"`, cursor 1
- Insert at end: `handler_with("hell", 4)` → `insert_char('o', &mut sink())` → buffer `"hello"`, cursor 5
- Insert in middle: `handler_with("hllo", 1)` → `insert_char('e', &mut sink())` → buffer `"hello"`, cursor 2
- Insert multibyte: `handler_with("cafe", 3)` → `insert_char('é', &mut sink())` → buffer `"caféfe"` wait no — `handler_with("caf", 3)` → `insert_char('é', &mut sink())` → buffer `"café"`, cursor 4

**`delete_range`:**
- Delete from start: `handler_with("hello", 0)` → `delete_range(0, 2, &mut sink())` → buffer `"llo"`, cursor 0
- Delete from middle: `handler_with("hello", 2)` → `delete_range(1, 3, &mut sink())` → buffer `"hlo"`, cursor 1
- Delete to end: `handler_with("hello", 3)` → `delete_range(3, 5, &mut sink())` → buffer `"hel"`, cursor 3
- Delete with multibyte: `handler_with("café", 3)` → `delete_range(3, 4, &mut sink())` → buffer `"caf"`, cursor 3

**`move_cursor`:**
- Move to 0, to end, to middle — verify cursor position after each.

### 7. `handle_key` integration tests (requires writer threading)

Test the full key dispatch via `handle_key(&key_event, &mut sink())`, verifying buffer and cursor state plus the returned `InputAction` variant:

- **Char insertion:** `KeyCode::Char('a')` on empty → buffer `"a"`, cursor 1, returns `None`
- **Backspace:** buffer `"ab"` cursor 2, Backspace → buffer `"a"`, cursor 1
- **Ctrl+A:** buffer `"hello"` cursor 3, Ctrl+A → cursor 0
- **Ctrl+E:** cursor 0 → cursor = char count
- **Ctrl+W:** buffer `"hello world"` cursor 11, Ctrl+W → buffer `"hello "`, cursor 6
- **Ctrl+U:** buffer `"hello"` cursor 3, Ctrl+U → buffer `"lo"`, cursor 0
- **Ctrl+K:** buffer `"hello"` cursor 2, Ctrl+K → buffer `"he"`, cursor 2
- **Alt+D:** buffer `"hello world"` cursor 0, Alt+D → buffer `" world"`, cursor 0
- **Enter:** returns `Submit(text, Steering)`
- **Alt+Enter:** returns `Submit(text, FollowUp)`
- **Esc:** returns `Cancel`, deactivates
- **Ctrl+C while active:** returns `Interrupt`

### 8. `handle_enter` / submit tests (requires writer threading)

- Empty buffer + Enter → returns `None` (no submit)
- View command (`:3`) + Enter → returns `ViewMessage("3")`
- Regular text + Enter → returns `Submit(text, Steering)`
- Regular text + Alt+Enter → returns `Submit(text, FollowUp)`

### 9. `handle_inactive_key` tests (requires writer threading)

- Ctrl+C when inactive → returns `Interrupt`
- Ctrl+D when inactive → returns `EndSession`
- Ctrl+O when inactive → returns `Interactive`
- Regular char when inactive → returns `Activated(c)`, handler becomes active, buffer contains char

### Verify

`cargo fmt && cargo clippy && cargo test` — all pass.
