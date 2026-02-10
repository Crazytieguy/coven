Issue: [P1] A single Ctrl+C Kills coven instead of just interrupting claude
Status: draft

## Approach

### Root cause

The code in `run.rs` and `ralph.rs` already intends to show a resumption prompt after Ctrl+C — `render_interrupted()` is called, then `wait_for_user_input()`. But it doesn't work because of a race with `ProcessExit`:

1. User presses Ctrl+C → `InputAction::Interrupt` → `runner.kill()` → SIGKILL to child
2. The child's stdout closes → background reader sends `ProcessExit` to the event channel
3. `run.rs`/`ralph.rs` calls `runner.close_input()` and `runner.wait()`, but neither drains the event channel
4. `render_interrupted()` prints "[interrupted]"
5. `wait_for_user_input()` → `wait_for_text_input()` → `io.next_event()` picks up the pending `ProcessExit`
6. `wait_for_text_input` line 403: `IoEvent::Claude(AppEvent::ProcessExit(_)) => return Ok(None)`
7. Returns `None` → caller `break`s → coven exits

The user sees "[interrupted]" flash briefly and then coven exits. The resumption prompt never appears.

### Fix

After killing the runner and waiting for it, call `io.replace_event_channel()` to discard the old channel (which has the stale `ProcessExit`). The returned sender can be dropped — there's no active reader until the user submits text and a new runner is spawned.

This is a one-line fix in each of `run.rs` and `ralph.rs`:

**`src/commands/run.rs`** (in the `SessionOutcome::Interrupted` arm, after `runner.wait()`):
```rust
SessionOutcome::Interrupted => {
    runner.close_input();
    let _ = runner.wait().await;
    drop(io.replace_event_channel());  // <-- add this
    let Some(session_id) = state.session_id.take() else {
        break;
    };
    // ... rest unchanged
}
```

**`src/commands/ralph.rs`** (same pattern — after `runner.wait()` in the Interrupted arm):
```rust
SessionOutcome::Interrupted => {
    // runner.close_input() and runner.wait() already called above
    drop(io.replace_event_channel());  // <-- add this
    let Some(session_id) = state.session_id.take() else {
        break 'outer;
    };
    // ... rest unchanged
}
```

### Testing

Create a VCR test that simulates Ctrl+C mid-stream, verifies the "[interrupted]" message appears, and verifies the prompt is shown for resumption. This exercises the `Interrupted` → `wait_for_user_input` path.

## Questions

None — the fix is mechanical and the root cause is clear.

## Review

