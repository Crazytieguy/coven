Issue: [P2] Fork: `flush_event_buffer` doesn't detect fork tags — if a Result event with a `<fork>` tag arrives while the user is typing (events are buffered), the fork will be silently skipped.
Status: draft

## Approach

The normal event path (`process_claude_event`, line 220) detects fork tags in `Result` events and runs the fork flow. But the buffered path (`flush_event_buffer`, line 293) only produces `Followup`/`Completed`/`ProcessExited` — it never checks for fork tags.

### Changes

1. **Add `Fork` variant to `FlushResult`** (session_loop.rs ~line 109):
   Add `Fork(Vec<String>)` to carry parsed fork task labels through the flush pipeline.

2. **Thread `fork_config` into `flush_event_buffer`** (session_loop.rs ~line 293):
   Add `fork_config: Option<&ForkConfig>` parameter. When a `Result` event is encountered and `fork_config` is `Some`, call `fork::parse_fork_tag(&locals.result_text)`. If a fork tag is found, set `result = FlushResult::Fork(tasks)` instead of `Followup`/`Completed`.

3. **Handle `FlushResult::Fork` in `handle_flush_result`** (session_loop.rs ~line 332):
   Add `fork_config: Option<&ForkConfig>` parameter. When `Fork(tasks)` is received, run `fork::run_fork`, send the reintegration message, set state to Running, and return `Ok(None)` (continue the loop).

4. **Update all call sites** of `flush_event_buffer` (3 sites at lines 138, 171, 192) and `handle_flush_result` (3 sites at lines 142, 177, 198) to pass through `fork_config` and `vcr` (for the `is_live()` gate — flush should respect the same live-mode-only check as the normal path, since fork children spawn real sessions).

### Notes

- The `vcr.is_live()` gate should apply here too (consistent with `process_claude_event`). Once the fork-vcr-support issue is resolved and the gate is removed there, it should be removed here as well.
- `flush_event_buffer` itself stays synchronous — the async fork work happens in `handle_flush_result`, which is already async.
- The `has_pending` flag in `flush_event_buffer` (line 302) should also account for detected fork tasks, so the "Done" line is suppressed (same as in `process_claude_event` line 241).

## Questions

### Should the ViewMessage (`:N`) and Cancel flush paths also handle forks?

Currently, `ViewMessage` (line 171) handles `Completed` specially by returning early, while `Cancel` (line 192) does similar. Both call `handle_flush_result` for other cases. Adding `Fork` to `handle_flush_result` means these paths will automatically handle fork too, which seems correct — if a fork tag arrived while the user was viewing a message or cancelling input, we should still run the fork.

The `Submit` path (line 138) has a comment noting that `Completed` is "intentionally not special-cased" — fork should similarly flow through `handle_flush_result` there.

Answer:

## Review

