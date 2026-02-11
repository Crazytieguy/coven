Issue: [P1] :N view in pager is bottom-aligned for messages smaller than the height of the terminal. Should be top aligned by default
Status: draft

## Approach

Pad the content with trailing newlines before piping to the pager so short messages fill the screen and appear top-aligned. This works with any pager (`$PAGER` or `less`) without changing flags or bypassing the pager.

### Change

In `view_message()` (`src/commands/session_loop.rs:502`):

1. After getting `content` from `format_message`, get terminal height via `crossterm::terminal::size()`
2. Count the number of newlines in `content`
3. If the line count is less than terminal height, append enough `\n` to fill the remaining rows
4. Pipe the padded content to the pager as before

This is a ~5 line change inside the existing function, and requires no new dependencies or control flow changes.

### Why this works

When `less` receives content shorter than the terminal, some terminal/less combinations show the content bottom-aligned on the alternate screen. Padding to fill the screen eliminates the ambiguity — the content occupies the full screen, so the first line is always at the top.

## Questions

## Review

