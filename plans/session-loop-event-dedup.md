Issue: Duplication between `process_claude_event` and `flush_event_buffer` in session_loop.rs: both have identical fork-detection, has_pending, handle_inbound, and result-handling logic. Extract a shared event-classification helper to reduce drift risk.
Status: draft

## Approach

The duplicated logic in `process_claude_event` (lines 234-284) and `flush_event_buffer`'s `AppEvent::Claude` branch (lines 312-339) share these identical steps:

1. Capture result text from `InboundEvent::Result`
2. Detect fork tag in result text
3. Compute `has_pending` (pending followups or fork tasks)
4. Call `handle_inbound` with the computed `has_pending`
5. Branch on fork/result/followup to determine next action

**Extract a `classify_claude_event` helper** that takes a `Box<InboundEvent>`, `locals`, `state`, `renderer`, and `fork_config`, performs steps 1-4, and returns an enum describing what action to take:

```rust
enum ClaudeEventAction {
    /// Normal event (not a Result), already rendered. No further action.
    Rendered,
    /// Result with fork tasks detected.
    Fork(Vec<String>),
    /// Result with a pending followup to send.
    Followup(String),
    /// Result with no followups — session completed.
    Completed(String),
}
```

Then:
- `process_claude_event` calls `classify_claude_event` and acts on the result (runs fork, sends followup, or returns `SessionOutcome::Completed`).
- `flush_event_buffer` calls `classify_claude_event` in its loop and maps the result to `FlushResult`.

The `ParseWarning` and `ProcessExit` branches are already trivial and don't need deduplication.

## Questions

### Should `classify_claude_event` be a method on `SessionLocals`?

Since it mutates `locals.result_text` and `locals.pending_followups`, it could be a method. But it also needs `state`, `renderer`, and `fork_config`. A free function with explicit parameters seems cleaner and more consistent with the existing style.

Answer:

## Review

