---
priority: P2
state: new
---

# `view_message` silently swallows all pager errors

`src/commands/session_loop.rs:520-542` — the `view_message()` function has multiple silent failure points:

1. **Line 526:** If the pager fails to spawn (e.g., `$PAGER` is set to a nonexistent binary), `.spawn()` returns `Err` but the entire `if let Ok(ref mut child)` block is skipped — the user sees nothing and gets no error message.

2. **Line 530:** `stdin.write_all(...).ok()` — if writing to the pager fails (broken pipe), the error is silently discarded.

3. **Line 534:** `child.wait().ok()` — pager exit status is ignored.

4. **Line 542:** `terminal::enable_raw_mode().ok()` — if re-enabling raw mode fails after the pager exits, the terminal is left in cooked mode. Subsequent input handling will break silently.

**Impact:** Users with a broken `$PAGER` configuration get no feedback when trying to inspect messages with `:N`. Terminal state corruption after pager failure is hard to diagnose.

**Fix:** At minimum, print an error line when the pager fails to spawn. For `enable_raw_mode`, consider logging the error or panicking since the session can't continue correctly without raw mode.
