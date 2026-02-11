Issue: [P1] When sending a user message that wraps on multiple lines, only the last line is cleared before the message is repeated.
Status: draft

## Approach

The bug is in `src/display/input.rs`. Both the Enter handler (line 110-116) and Escape handler (line 139-145) use `\r` + `Clear(CurrentLine)`, which only clears the line the cursor is currently on. When a message is long enough to wrap in the terminal, the cursor is on the last wrapped line, so only that line gets cleared — the earlier wrapped lines remain on screen, causing a visual duplicate.

### Fix

Before clearing, calculate how many terminal lines the input occupies and move the cursor up accordingly:

```rust
fn clear_input_lines(buffer: &str) {
    let mut out = io::stdout();
    let term_width = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
    // The "> " prefix is 2 chars
    let input_display_len = buffer.len() + 2;
    let lines_occupied = input_display_len.div_ceil(term_width).max(1);
    if lines_occupied > 1 {
        queue!(out, cursor::MoveUp((lines_occupied - 1) as u16)).ok();
    }
    queue!(
        out,
        crossterm::style::Print("\r"),
        terminal::Clear(terminal::ClearType::FromCursorDown),
    )
    .ok();
    out.flush().ok();
}
```

Key changes:
- Use `terminal::size()` to get the current terminal width
- Calculate how many wrapped lines the input occupies (buffer length + 2 for `"> "` prefix, divided by terminal width, rounded up)
- Move cursor up to the first line of input
- Use `Clear(FromCursorDown)` instead of `Clear(CurrentLine)` to clear all occupied lines

Extract this into a helper function and call it from both the Enter and Escape handlers.

### Related issue: Backspace across line wraps

The Backspace handler (line 94-98) uses `cursor::MoveLeft(1)`, which doesn't cross line boundaries — pressing backspace at the start of a wrapped line won't move to the end of the previous line. This is a separate but related issue. The fix would be to detect when the cursor is at column 0 and use `MoveUp(1)` + `MoveToColumn(term_width - 1)` instead. Not in scope for this plan but worth noting.

## Questions

### Should the "> " prefix width be a constant or derived from context?

Currently the prefix is hardcoded in the caller (session_loop.rs renders `"> "`). Using a constant `2` here couples the two. Options:
1. Hardcode `2` — simple, matches current behavior
2. Pass the prefix width into `InputHandler` — more robust if the prefix changes

Answer:

## Review

