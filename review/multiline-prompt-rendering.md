---
priority: P2
state: review
---

# Multi-line typing in interactive prompt renders incorrectly

When typing multi-line input in the interactive prompt, the display behaves incorrectly at line transitions. Likely an off-by-one error in line wrapping or cursor positioning logic.

## Plan

### Root Cause

The bug is a terminal "pending wrap" (deferred autowrap) issue in `src/display/input.rs`, specifically in `redraw()`.

When the total display width (prefix + buffer) is an exact multiple of the terminal width, the terminal cursor enters a **pending wrap state** instead of advancing to the next line. The code assumes the cursor has wrapped, causing `end_line` (and subsequently `term_cursor_display`) to be off by one.

**Concrete example:** Terminal width = 80, prefix = `"> "` (2 cols), user types 78 characters (total_display = 80):

1. `redraw()` prints 80 characters. Terminal cursor is in pending wrap state at end of line 0 — it has NOT moved to line 1.
2. Code calculates `end_line = 80 / 80 = 1` — **wrong**, cursor is on line 0.
3. `target_line = 80 / 80 = 1`, `target_col = 0`, `lines_up = 0`.
4. `MoveToColumn(0)` resolves the pending wrap to column 0 of line 0 (not line 1).
5. `term_cursor_display` is set to 80, recording the cursor as being on line 1.
6. **Next redraw:** `cur_line = 80 / 80 = 1`, so `MoveUp(1)` is issued — but cursor is on line 0, causing display corruption.

This affects every line transition where the text fills a line exactly (every `tw` characters past the prefix).

### Fix

In `redraw()`, after printing the buffer, force the terminal to resolve any pending wrap by emitting a space character (which triggers the actual line wrap) followed by `cursor::MoveLeft(1)` (which moves back to column 0 of the new line). This is the standard technique used by readline-style editors.

**Changes to `src/display/input.rs`, `redraw()` method only:**

1. Move the `total_display` computation above the print block (it's currently below it).
2. Split the `queue!` that prints buffer + clears into separate operations.
3. Between the print and the clear, add the force-wrap: if `total_display > 0 && total_display % tw == 0`, emit `Print(" ")` + `MoveLeft(1)`.

After the fix, `end_line = total_display / tw` is always correct because the pending wrap has been resolved, and the cursor is definitively on the expected line. No changes needed to `clear_input_lines` or anywhere else — that function reads `term_cursor_display` which will now be set correctly.

```rust
// BEFORE (lines 130-145):
queue!(
    out,
    crossterm::style::Print(theme::prompt_style().apply("> ")),
    crossterm::style::Print(&self.buffer),
    terminal::Clear(terminal::ClearType::FromCursorDown),
).ok();

let byte_pos = self.cursor_byte_pos();
let new_cursor_display = self.prefix_width + self.buffer[..byte_pos].width();
let total_display = self.prefix_width + self.buffer.width();
let end_line = total_display / tw;

// AFTER:
let total_display = self.prefix_width + self.buffer.width();

queue!(
    out,
    crossterm::style::Print(theme::prompt_style().apply("> ")),
    crossterm::style::Print(&self.buffer),
).ok();

// When total display width is an exact multiple of terminal width, the
// terminal cursor is in "pending wrap" state rather than on the next line.
// Print a space to force the wrap to resolve, then move back.
if total_display > 0 && total_display % tw == 0 {
    queue!(out, crossterm::style::Print(" "), cursor::MoveLeft(1)).ok();
}

queue!(out, terminal::Clear(terminal::ClearType::FromCursorDown)).ok();

let byte_pos = self.cursor_byte_pos();
let new_cursor_display = self.prefix_width + self.buffer[..byte_pos].width();
let end_line = total_display / tw;
```

### Testing

This is a visual terminal rendering fix that can't be verified through VCR tests. Manual testing:

1. `cargo build && cargo clippy && cargo test` — ensure no regressions.
2. Run `coven` in a terminal, type a message long enough to wrap to a second line. Verify the cursor and text display correctly at the transition point.
3. Continue typing past the second line boundary. Verify continued correct behavior.
4. Use backspace/arrow keys to navigate across line boundaries.
5. Press Escape and Enter to verify clearing works correctly with wrapped text.
