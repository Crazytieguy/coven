---
priority: P1
state: approved
---

# Ctrl+O keybinding in interrupted state doesn't work

The Ctrl+O keybinding in interrupted state doesn't work — it just types "o" instead of performing the bound action.

## Plan

**Root cause:** `wait_for_text_input()` in `session_loop.rs:500` calls `input.activate()`, putting the `InputHandler` into active mode. When active, `handle_key()` (`input.rs:222`) dispatches to the active-mode match block (lines 231–327), which has no `Ctrl+O` arm. The keypress falls through to the catch-all `KeyCode::Char(c)` at line 307, inserting `'o'` into the buffer.

The inactive-mode handler (`handle_inactive_key`, line 335) does have the binding: `KeyCode::Char('o') if ctrl => InputAction::Interactive`.

**Fix:** Add `KeyCode::Char('o') if ctrl => InputAction::Interactive` to the active-mode match block in `handle_key()`, right after the existing `Ctrl+D` arm (line 233). This mirrors the inactive handler and ensures Ctrl+O works regardless of whether the input is active.

Note: this also makes Ctrl+O work while typing a follow-up message (not just the interrupted state), since both paths go through `wait_for_text_input`. This seems correct — the user should be able to switch to interactive mode at any point.
