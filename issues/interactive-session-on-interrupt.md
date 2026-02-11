---
priority: P1
state: approved
---

# Keybinding to open interactive session from interrupted state

After interrupting a coven session (Ctrl+C), the user should be able to press a key to drop into the native Claude Code TUI (`claude --resume <session_id>`, no `-p`), continuing the same conversation interactively. When the user exits the native TUI, they return to coven's interrupted state where they can resume non-interactively with a prompt or exit.

No need to clear the display — whatever the interactive session left on screen is fine.

Should work from the interrupted state in run, ralph, and worker.

## Plan

### Overview

Add a new `InputAction::Interactive` variant that triggers from the interrupted state. When received, temporarily exit raw mode, spawn `claude --resume <session_id>` as a blocking child process (the native TUI), wait for it to exit, re-enable raw mode, and return to the interrupted prompt loop.

### 1. Add `InputAction::Interactive` variant

**File:** `src/display/input.rs`

Add `Interactive` to the `InputAction` enum (after `EndSession`):

```rust
/// User wants to drop into native Claude TUI.
Interactive,
```

In `handle_inactive_key`, add a match arm before the catch-all `KeyCode::Char(c)` arm. Use `Ctrl+O` as the trigger key ("open"):

```rust
KeyCode::Char('o') if ctrl => InputAction::Interactive,
```

### 2. Handle `Interactive` in `wait_for_text_input`

**File:** `src/commands/session_loop.rs`

In the `wait_for_text_input` match on `action`, add a new arm for `InputAction::Interactive`. This arm needs to:

1. Return a new enum value so the caller knows to shell out.

Change `wait_for_text_input` (and `wait_for_user_input`) to return `Result<Option<WaitResult>>` instead of `Result<Option<String>>`, where:

```rust
pub enum WaitResult {
    Text(String),
    Interactive,
}
```

The `Interactive` arm returns `Ok(Some(WaitResult::Interactive))`. The `Submit` arm returns `Ok(Some(WaitResult::Text(text)))`. The exit arms return `Ok(None)` as before.

Update `wait_for_followup` to destructure `WaitResult::Text` (the `Interactive` variant can't occur there since the input is active, but handle it as a no-op / continue the loop for safety).

### 3. Shell out to native Claude TUI

**File:** `src/commands/session_loop.rs`

Add a public function:

```rust
pub async fn open_interactive_session(
    session_id: &str,
    working_dir: Option<&Path>,
    extra_args: &[String],
) -> Result<()>
```

Implementation:
- Disable raw mode (`crossterm::terminal::disable_raw_mode()`)
- Print a hint line: `"\r\n[opening interactive session — exit to return]\r\n"`
- Build `std::process::Command::new("claude")` with args `["--resume", session_id]` plus all `extra_args` (filtering out only `-p` and `--output-format` since those are for non-interactive mode)
- Set working directory if provided
- Inherit stdio (stdin/stdout/stderr)
- Error if VCR is not live (no VCR support needed for this feature)
- Spawn and wait for the child process
- Re-enable raw mode (`crossterm::terminal::enable_raw_mode()`)
- Print `"\r\n[returned to coven]\r\n"` after re-enabling raw mode

### 4. Handle `WaitResult::Interactive` in each mode's interrupt handler

In `run.rs`, `ralph.rs`, and `worker.rs`, where `wait_for_user_input` is called inside the `SessionOutcome::Interrupted` arm:

Replace:
```rust
let Some(text) = session_loop::wait_for_user_input(...).await? else { break; };
```

With a loop that handles both results:
```rust
loop {
    let result = session_loop::wait_for_user_input(...).await?;
    match result {
        Some(WaitResult::Text(text)) => { /* existing resume logic */ break; }
        Some(WaitResult::Interactive) => {
            session_loop::open_interactive_session(&session_id, ..., vcr).await?;
            renderer.render_interrupted(); // re-print the [interrupted] indicator
            continue; // back to waiting for input
        }
        None => { /* existing exit logic */ break; }
    }
}
```

The session ID and working directory are already available in scope in all three modes. Pass `extra_args` from the session config.

### 5. Show the keybinding hint

**File:** `src/display/renderer.rs`

Update `render_interrupted()` to include the hint. Currently it prints `"[interrupted]"` — change it to print `"[interrupted — Ctrl+O to open interactive]"` (or similar short text) so the user knows about the keybinding.

## Resolved questions

1. **Keybinding:** Ctrl+O ("open").
2. **Args:** Forward all args except `-p` and `--output-format`.
3. **VCR:** No VCR support — error if not live.
