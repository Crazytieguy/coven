Issue: :N command: pager displays the content at the bottom of the terminal instead of the top
Status: draft

## Approach

The current `view_message()` in `src/commands/session_loop.rs` (lines 323-362) manually enters the alternate screen before spawning `less`, then leaves it after. The problem is that `less` also manages its own alternate screen — so we get a double alternate-screen situation where cursor positioning becomes unreliable, causing content to appear at the bottom.

### Fix

Remove the manual `EnterAlternateScreen`/`LeaveAlternateScreen` calls and let the pager handle its own screen management. The pager (`less`, `bat`, `more`, etc.) is designed to manage the terminal display itself.

**Before:**
```rust
crossterm::execute!(std::io::stdout(), terminal::EnterAlternateScreen).ok();
terminal::disable_raw_mode().ok();
// ... spawn pager ...
terminal::enable_raw_mode().ok();
crossterm::execute!(std::io::stdout(), terminal::LeaveAlternateScreen).ok();
```

**After:**
```rust
terminal::disable_raw_mode().ok();
// ... spawn pager ...
terminal::enable_raw_mode().ok();
```

That's it — just remove the two `execute!` calls. The pager owns the screen for its lifetime, and our only responsibility is toggling raw mode so the pager can handle keyboard input.

### Files to change

- `src/commands/session_loop.rs` — remove `EnterAlternateScreen`/`LeaveAlternateScreen` from `view_message()`

## Questions

### If the user's pager doesn't use alternate screen, won't it pollute coven's display?

If someone sets `LESS=-X` (which disables alternate screen in `less`), the pager output will appear inline and overwrite coven's streaming display. But that's the user's explicit choice — they've configured their pager to behave that way. We shouldn't override their preference by wrapping it in our own alternate screen.

The alternative fix (keep our alternate screen but add `Clear(All)` + `MoveTo(0,0)` after entering it) would preserve the current behavior while fixing the positioning — but risks the double-alternate-screen issue on terminals where `less` also enters one.

Should we go with the simple "remove our alternate screen" approach, or the "keep it but clear" approach?

Answer:

## Review

