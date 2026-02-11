Issue: [P1] :N view in pager is bottom-aligned for messages smaller than the height of the terminal. Should be top aligned by default
Status: draft

## Approach

When short content is piped to `less`, some terminal/less combinations show it bottom-aligned (content hugged to the bottom of the alternate screen). Fix by bypassing the pager entirely for content that fits on one screen.

### Change

In `view_message()` (`src/commands/session_loop.rs:502`):

1. Get terminal height via `crossterm::terminal::size()`
2. Count content lines (accounting for line wrapping using terminal width)
3. If content fits on screen: display inline using `write_raw()`, then wait for any keypress to return (use crossterm `read()` for a single `Event::Key`)
4. If content exceeds screen: use `less -R` as today

The inline display should:
- Clear the screen and move cursor to top-left (`\x1b[2J\x1b[H`)
- Print the content
- Show a dim "press any key to continue" footer
- Wait for one keypress
- Clear the screen again to return to the session display

This avoids all pager-specific alignment quirks for short messages while preserving `less` for genuinely long content.

### Line counting

Use terminal width to account for wrapping: each content line takes `ceil(display_width / terminal_width)` rows. Use `unicode_width::UnicodeWidthStr` for display width (already a dependency). Reserve 2 rows for the footer.

## Questions

### Should we still support $PAGER for long content?

Current code respects `$PAGER`. Should the fallback (long content) continue to use `$PAGER`, or always use `less`?

Leaning toward keeping `$PAGER` support for long content — users who set it expect it to be used.

Answer:

## Review

