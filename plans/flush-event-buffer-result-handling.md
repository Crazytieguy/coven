Issue: [P0] flush_event_buffer mishandles Result and ProcessExit events — Done line shown with pending followups, followup queue order violated, dead-process send on buffered exit
Status: approved


## Approach

The root cause is that `flush_event_buffer` in `session_loop.rs` processes events differently from `process_claude_event`:

1. **Always passes `has_pending_followups: false`** to `handle_inbound`, so `render_result()` shows the "Done" line even when pending followups exist (in `process_claude_event`, the Done line is suppressed when followups are queued).

2. **Doesn't dispatch pending followups** when a Result is flushed. In `process_claude_event`, a Result event triggers sending the next queued followup. In the flush path, this is skipped. The Submit handler then sends the *new* message directly (because state is now WaitingForInput), jumping the queue ahead of older pending followups. Order ends up: newest message first, then older queued messages.

3. **Doesn't signal session end on ProcessExit**. If a ProcessExit event is buffered and flushed, state becomes `Ended` but the caller doesn't know — it continues trying to send messages to the dead process.

### Reproduction scenario (ordering bug)

1. Claude is running. User presses Alt+Enter to queue followup A.
2. Claude sends Result → gets buffered (user is typing).
3. User submits followup B → flush runs, state becomes WaitingForInput.
4. B is sent directly (line 127), bypassing the queue.
5. Claude finishes B, sends new Result → `process_claude_event` dequeues A.
6. Final order: B, A (should have been A, B).

### Fix

Refactor `flush_event_buffer` to return an `Option<SessionOutcome>`, mirroring `process_claude_event`'s handling:

- Pass `!locals.pending_followups.is_empty()` as `has_pending_followups` when calling `handle_inbound` for Result events.
- When a Result is flushed and there are pending followups, dequeue and send the next one (requires making the function async, or collecting the followup text to send after the flush).
- When a ProcessExit is flushed, return `Some(SessionOutcome::ProcessExited)`.
- When a Result is flushed with no pending followups, return `Some(SessionOutcome::Completed)`.

The callers (`Submit`, `ViewMessage`, `Cancel`) should check the returned outcome and return early if the session ended during the flush.

A simpler alternative: after the flush completes, check whether `state.status` changed to `WaitingForInput` or `Ended`, and handle those transitions in the caller. This avoids making the flush async but requires each caller to duplicate the post-flush logic.

## Questions

### Should flush_event_buffer become async?

Dispatching a queued followup during the flush requires calling `runner.send_message().await`. This means either:
- (A) Make `flush_event_buffer` async and pass `runner` to it.
- (B) Have `flush_event_buffer` return the followup text, and let the caller send it.

Option B is simpler and keeps the flush function focused on rendering. The caller already has access to `runner`.

Answer: B

## Review

