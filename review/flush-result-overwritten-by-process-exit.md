---
priority: P1
state: review
---

# flush_event_buffer loses Result when ProcessExit follows

`flush_event_buffer` in `src/commands/session_loop.rs:334-365` uses "last one wins" semantics: as it iterates through buffered events, each significant event overwrites the previous `FlushResult`. When a `Result` event and `ProcessExit` event are both buffered during user input, `ProcessExit` overwrites `Completed`.

## How it happens

1. User starts typing (input handler is active, events get buffered)
2. Claude session completes — `Result` event arrives and is buffered
3. Claude process exits (stdout closes) — `ProcessExit` event arrives and is buffered
4. User submits text, triggering `flush_event_buffer`
5. Buffer processes `Result` → `result = FlushResult::Completed(text)`
6. Buffer processes `ProcessExit` → `result = FlushResult::ProcessExited` (overwrites)
7. `handle_flush_result` receives `ProcessExited`, returns `SessionOutcome::ProcessExited`

The session's result text (and the fact it completed successfully) is lost.

## Impact

- In `run` mode: session ends without follow-up prompt, minor UX issue
- In `worker` mode: `run_phase_session` returns `PhaseOutcome::Exited` (worker.rs:1011-1013), causing the worker to skip landing and exit. If the agent committed work, that work is lost when the worktree is removed during cleanup.

## Where

- `src/commands/session_loop.rs:334-365` — `flush_event_buffer` function
- `src/commands/session_loop.rs:369-403` — `handle_flush_result` function

## Fix

`flush_event_buffer` should not allow `ProcessExited` to override `Completed` or `Followup`. A `Result` event followed by a `ProcessExit` is the normal completion sequence — the completion should take precedence. One approach: skip the `ProcessExited` assignment if `result` is already `Completed` or `Followup` or `Fork`.

## Plan

In `src/commands/session_loop.rs`, in the `flush_event_buffer` function (~line 356-360), change the `AppEvent::ProcessExit` arm from unconditionally overwriting `result` to only setting `ProcessExited` when `result` is still `Continue`. The current code:

```rust
AppEvent::ProcessExit(code) => {
    renderer.render_exit(code);
    state.status = SessionStatus::Ended;
    result = FlushResult::ProcessExited;
}
```

becomes:

```rust
AppEvent::ProcessExit(code) => {
    renderer.render_exit(code);
    state.status = SessionStatus::Ended;
    if matches!(result, FlushResult::Continue) {
        result = FlushResult::ProcessExited;
    }
}
```

This means `ProcessExited` only wins when nothing meaningful preceded it in the buffer. If a `Result` (yielding `Completed`, `Followup`, or `Fork`) was already processed, the process exit is treated as the expected follow-on — the exit code is still rendered and status still set to `Ended`, but the `FlushResult` preserves the completion info.

No other files need changes.
