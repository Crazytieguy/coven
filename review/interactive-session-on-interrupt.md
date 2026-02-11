---
priority: P1
state: review
---

# Keybinding to open interactive session from interrupted state

Add a keybinding that enters an interactive session with Claude after interrupting (resumed). Exiting the session puts you back in the interrupted state. Should seamlessly integrate with run, ralph, and worker.

## Plan

### Overview

After interrupting a session (Ctrl+C), the user enters `wait_for_user_input` where they can type a resume message or exit. This plan adds a **Ctrl+O** keybinding in that state that opens a fresh interactive session. When the interactive session ends, the user returns to the interrupted prompt and can resume, open another interactive session, or exit.

### Keybinding: Ctrl+O

Ctrl+O ("Open") is unused in both active and inactive input modes and has no standard terminal conflict. It's handled in `InputHandler::handle_key()` for both active (empty buffer) and inactive states.

### Changes

#### 1. `src/display/input.rs` — New `InputAction::OpenInteractive` variant

- Add `OpenInteractive` to the `InputAction` enum.
- In `handle_inactive_key()`: add `KeyCode::Char('o') if ctrl => InputAction::OpenInteractive`.
- In `handle_key()` (active mode): add `KeyCode::Char('o') if ctrl && self.buffer.is_empty()` → deactivate, clear input lines, return `InputAction::OpenInteractive`. Only when buffer is empty to avoid losing typed text.

#### 2. `src/commands/session_loop.rs` — Return `OpenInteractive` from `wait_for_text_input`

- Change `wait_for_text_input` return type from `Result<Option<String>>` to `Result<WaitResult>` (new enum):
  ```rust
  pub enum WaitResult {
      Text(String),
      Exit,
      OpenInteractive,
  }
  ```
- Handle `InputAction::OpenInteractive` in the event loop: return `WaitResult::OpenInteractive`.
- Update `wait_for_user_input` to return `Result<WaitResult>` (same type).
- Update `wait_for_followup` which calls `wait_for_text_input`: map `OpenInteractive` the same as a follow-up action or propagate it. Since `wait_for_followup` is only called in `run.rs` on normal completion (not interruption), it can treat `OpenInteractive` like `Exit` — the keybinding is really meant for the interrupted state, not the "waiting for follow-up" state. Alternatively, it could also support it there for consistency; decide based on what feels natural.

#### 3. `src/commands/run.rs` — Handle `OpenInteractive` in interrupt branch

In the `SessionOutcome::Interrupted` arm, wrap the `wait_for_user_input` call in a loop:
```rust
loop {
    match session_loop::wait_for_user_input(&mut input, &mut renderer, io, vcr).await? {
        WaitResult::Text(text) => {
            // existing resume logic
            break;
        }
        WaitResult::Exit => break_outer = true; break;
        WaitResult::OpenInteractive => {
            run_interactive_side_session(
                &base_session_cfg, &mut renderer, &mut input, io, vcr, fork_config.as_ref()
            ).await?;
            renderer.render_interrupted(); // re-show [interrupted]
            // loop continues → re-show prompt via wait_for_user_input
        }
    }
}
```

#### 4. `src/commands/ralph.rs` — Same pattern in interrupt branch

Wrap the `wait_for_user_input` in the `Interrupted` arm in a loop, identical pattern to run.rs.

#### 5. `src/commands/worker.rs` — Same pattern in `run_phase_session`

Wrap the `wait_for_user_input` in the `Interrupted` arm of `run_phase_session` in a loop, same pattern. Worker sessions use the worktree's `working_dir`, so the side session should inherit that.

#### 6. `src/commands/session_loop.rs` — New `run_interactive_side_session` function

Add a shared helper that all three callers use:

```rust
pub async fn run_interactive_side_session<W: Write>(
    base_config: &SessionConfig,
    renderer: &mut Renderer<W>,
    input: &mut InputHandler,
    io: &mut Io,
    vcr: &VcrContext,
    fork_config: Option<&ForkConfig>,
) -> Result<()>
```

This function:
1. Calls `wait_for_user_input` to get the initial prompt from the user (returning early if the user exits instead).
2. Spawns a session with the prompt using a `SessionConfig` cloned from `base_config` (inherits `extra_args`, `working_dir`, `append_system_prompt` — but **no** `resume`, since this is a fresh session).
3. Runs `run_session` in a loop handling `Completed` (wait for follow-up or exit) and `Interrupted` (re-show interrupted, wait for input within this side session too).
4. When the user exits (Ctrl+D / Ctrl+C from a prompt state), returns `Ok(())`.

This avoids calling `run::run()` which creates its own `RawModeGuard` and `Renderer` — we reuse the existing ones.

#### 7. `src/display/renderer.rs` — Help text update

Update `render_help` to include the new keybinding:
```
:N view message · type to steer · Alt+Enter follow up · Ctrl+O side session · Ctrl+D exit
```

Or show the Ctrl+O hint specifically in `render_interrupted`:
```
[interrupted] (Ctrl+O for side session)
```

#### 8. VCR considerations

The new `run_interactive_side_session` function uses existing VCR-wrapped primitives (`spawn_session`, `run_session`, `wait_for_user_input`, `vcr_send_message`), so VCR recording/replay will work out of the box. No new VCR operations needed.

A VCR test should be added (e.g. `tests/cases/interactive/side_session/` or similar) that records: interrupt → Ctrl+O → type prompt → get response → Ctrl+D → resume original. This validates the full flow.

### Order of implementation

1. Add `InputAction::OpenInteractive` + keybinding in `input.rs`
2. Add `WaitResult` enum and update `wait_for_text_input` / `wait_for_user_input` in `session_loop.rs`
3. Add `run_interactive_side_session` helper in `session_loop.rs`
4. Update `run.rs` interrupt handler to loop and handle `OpenInteractive`
5. Update `ralph.rs` interrupt handler
6. Update `worker.rs` `run_phase_session` interrupt handler
7. Update `wait_for_followup` to handle the new return type
8. Update help text in `renderer.rs`
9. Add VCR test

## Questions

1. **Keybinding**: Is Ctrl+O the right choice, or would you prefer a different key?
2. **Fresh vs resumed session**: The plan assumes the side session is a **fresh** session (new conversation, inheriting `extra_args` and `working_dir`). The issue mentions "(resumed)" — should the side session instead **resume** the interrupted session's conversation (using `--resume <session_id>`)? A fresh session seems more useful for "side conversations" while a resumed session would be more like the existing resume behavior but with multi-turn support before returning.
3. **Help text**: Should the Ctrl+O hint appear in the general help line, in the `[interrupted]` message, or both?
4. **`wait_for_followup` (completion state)**: Should Ctrl+O also work after a session completes normally (in the follow-up prompt), or only in the interrupted state?
