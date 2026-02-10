Issue: [P1] keys typed while the :N pager is active seem to make it into the input prompt when the pager is exited. Instead, when the pager is exited always exit the prompt (showing buffered messages)
Status: draft

## Approach

### Root cause

When the pager (less) runs, `view_message()` disables raw mode so less can control the terminal. Keys typed after less exits but before raw mode is re-enabled are line-buffered by the kernel. Additionally, the crossterm EventStream background task may pick up stray key events. When control returns to the event loop and `input.activate()` is called, those buffered keystrokes appear as input characters.

### Fix (3 changes)

**1. Flush the terminal input buffer in `view_message()` (`session_loop.rs:437`)**

Before re-enabling raw mode, call `libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH)` to discard any characters buffered in the kernel's terminal input queue. This eliminates the stale keystrokes at the source. `libc` is already a dependency.

```rust
// After child.wait() and before enable_raw_mode:
unsafe { libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH); }
terminal::enable_raw_mode().ok();
```

**2. Don't re-activate input after pager in `handle_session_key_event()` (`session_loop.rs:171-174`)**

Remove the block that re-shows the prompt and re-activates input after the pager:

```rust
// Remove:
if state.status == SessionStatus::WaitingForInput {
    renderer.show_prompt();
    input.activate();
}
```

Input was already deactivated when Enter was pressed to submit the `:N` command. After the pager, the user is back in inactive state — they can type a character to start a new input naturally (via `Activated(c)`).

**3. Don't re-activate input after pager in `wait_for_text_input()` (`session_loop.rs:388-392`)**

Remove `renderer.show_prompt()` and `input.activate()` after the pager:

```rust
// Change from:
InputAction::ViewMessage(ref query) => {
    view_message(renderer, query);
    renderer.show_prompt();
    input.activate();
}
// To:
InputAction::ViewMessage(ref query) => {
    view_message(renderer, query);
}
```

And handle `Activated(c)` in `wait_for_text_input()` (currently it's a no-op at line 400). When the user types their first character after the pager closes, show the prompt and the character:

```rust
// Change from:
InputAction::Activated(_) | InputAction::None => {}
// To:
InputAction::Activated(c) => {
    renderer.begin_input_line();
    write!(renderer.writer(), "{c}").ok();
    renderer.writer().flush().ok();
}
InputAction::None => {}
```

This mirrors the existing `Activated(c)` handling in `handle_session_key_event()`.

### Testing

This is a terminal interaction (pager + raw mode + key buffering) that can't be VCR-recorded. Manual verification:
1. Start a session, wait for output
2. Type `:1` to open pager, mash some keys, quit pager
3. Verify no stale characters appear in the prompt
4. Verify typing works normally after pager exit

## Questions

### Use `tcflush` directly or add `nix` crate?

`libc::tcflush` requires an `unsafe` block (trivially safe — it's just a POSIX syscall). Alternatively we could add the `nix` crate which provides a safe wrapper (`nix::unistd::tcflush`). Using `libc` directly avoids a new dependency and the `unsafe` is minimal and clear.

Recommendation: use `libc` directly.

Answer:

## Review

