Issue: [P1] Regular terminal keyboard navigation doesn't work when giving interactive input. It would be nice to be able to use the regular terminal keybindings for things like jumping back a word or deleting a word etc
Status: draft

## Approach

Add cursor position tracking and readline-like keybindings to `InputHandler` in `src/display/input.rs`.

### Data model change

Add `cursor: usize` field to `InputHandler` (char index into buffer, 0 = start). Reset to 0 in `activate()`/`deactivate()`.

### Redraw strategy

Replace per-keystroke incremental writes with a full-line redraw helper:

```
fn redraw(&self) {
    // \r → print "> " → print buffer → clear to EOL → move cursor to correct column
}
```

This is simple, correct for all operations, and avoids the complexity of partial redraws. The prompt prefix "> " is 2 display columns wide. Use `unicode_width::UnicodeWidthStr` for correct cursor positioning with non-ASCII text.

Character insertion (currently `push`) becomes `insert` at cursor position. Backspace becomes `remove` at cursor-1. The existing `Char(c)` and `Backspace` handlers switch to calling `redraw()` instead of doing their own queue! operations.

### Keybindings to add

**Cursor movement:**
- `Left` — move cursor left one char
- `Right` — move cursor right one char
- `Ctrl+A` / `Home` — move to start of line
- `Ctrl+E` / `End` — move to end of line
- `Alt+B` / `Ctrl+Left` — move back one word
- `Alt+F` / `Ctrl+Right` — move forward one word

**Deletion:**
- `Delete` — delete char at cursor
- `Ctrl+W` — delete word backward
- `Alt+Backspace` — delete word backward (same as Ctrl+W)
- `Ctrl+U` — delete from cursor to start of line
- `Ctrl+K` — delete from cursor to end of line
- `Alt+D` — delete word forward

**Word boundary** — a word boundary is a transition between whitespace and non-whitespace (standard readline word definition). Implement two helpers: `word_boundary_left(&self) -> usize` and `word_boundary_right(&self) -> usize` that return the char index of the nearest word boundary in each direction.

### What NOT to change

- The `Activated(char)` flow stays the same — caller still handles `begin_input_line()` before the first redraw
- Submit (Enter/Alt+Enter), Cancel (Esc), Interrupt (Ctrl+C), EndSession (Ctrl+D) are unchanged
- No kill ring / yank (Ctrl+Y) — that's a nice-to-have for later
- No history (up/down arrows) — out of scope

### Testing

This is all terminal rendering logic, so no VCR test changes needed. The behavior is best verified manually. The existing unit tests for `parse_view_command` are unaffected.

## Questions

### Should Ctrl+A conflict with the potential future "select all" semantic?

In readline, Ctrl+A means "beginning of line." In some editors it means "select all." Since this is a single-line input in a terminal, readline semantics are clearly correct here. But flagging in case you have other plans for Ctrl+A.

Answer:

### Alt key detection on macOS

On macOS Terminal.app, Alt (Option) key doesn't always send proper Alt-modified key events — it sends Unicode characters instead (e.g., Option+B sends `∫`). iTerm2 and most modern terminals can be configured to send proper escape sequences. crossterm handles this correctly for terminals that send proper sequences. Should we add a note about this in the README, or just rely on crossterm's handling?

Answer:
