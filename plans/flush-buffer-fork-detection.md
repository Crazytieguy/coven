Issue: [P2] Fork: `flush_event_buffer` doesn't detect fork tags — if a Result event with a `<fork>` tag arrives while the user is typing (events are buffered), the fork will be silently skipped.
Status: draft

## Approach

The normal event path (`process_claude_event`, line 212) detects fork tags in `Result` events via `fork_config.and_then(|_| fork::parse_fork_tag(...))` and runs `fork::run_fork`. The buffered path (`flush_event_buffer`, line 291) only produces `Followup`/`Completed`/`ProcessExited` — it never checks for fork tags. Since fork VCR recording/replay is now fully supported (no `is_live()` gate), the fix is straightforward.

### Changes

1. **Add `Fork` variant to `FlushResult`** (session_loop.rs line 109):
   Add `Fork(Vec<String>)` to carry parsed fork task labels through the flush pipeline.

2. **Thread `fork_config` into `flush_event_buffer`** (session_loop.rs line 291):
   Add `fork_config: Option<&ForkConfig>` parameter. When a `Result` event is encountered and `fork_config` is `Some`, call `fork::parse_fork_tag(&locals.result_text)`. If a fork tag is found, set `result = FlushResult::Fork(tasks)` instead of `Followup`/`Completed`. Also set `has_pending = true` for the `handle_inbound` call so the "Done" line is suppressed (matching `process_claude_event` line 236).

3. **Handle `FlushResult::Fork` in `handle_flush_result`** (session_loop.rs line 330):
   Add `fork_config: Option<&ForkConfig>` and `state: &mut SessionState` parameters (state is needed for `session_id`). When `Fork(tasks)` is received:
   - Get `session_id` from `state.session_id`
   - Call `fork::run_fork(&session_id, tasks, fork_cfg, renderer, vcr).await`
   - Send the reintegration message via `vcr.call("send_message", ...)`
   - Set `state.suppress_next_separator = true` and `state.status = SessionStatus::Running`
   - Return `Ok(None)` (continue the loop)

   Note: `handle_flush_result` already takes `state` and `vcr` — only `fork_config` needs to be added.

4. **Update all call sites** — three sites call `flush_event_buffer` (lines 138, 171, 192) and `handle_flush_result` (lines 142, 177, 198). Pass `fork_config` to both. Since `handle_session_key_event` doesn't currently take `fork_config`, add it as a parameter and thread it from `run_session` (line 82).

### Fork handling at each call site

- **Submit** (line 137): `handle_flush_result` already handles `Followup` and `ProcessExited`; `Fork` flows through the same way. The existing comment about `Completed` not being special-cased still applies — after a fork completes, the session continues running, so the user's submitted text will be sent as a steering/followup message as normal.

- **ViewMessage** (line 169): Currently returns early on `Completed`. A `Fork` should NOT return early — the fork should run and the session continues. The existing `handle_flush_result` call on line 177 will handle it.

- **Cancel** (line 191): Same pattern as ViewMessage. Fork flows through `handle_flush_result` on line 198.

### Testing

This is difficult to test end-to-end via VCR because it requires the input to be active when a Result event arrives (a timing-dependent race). Two practical options:

1. **Unit test `flush_event_buffer` in isolation**: construct a `SessionLocals` with a buffered `Result` event containing a fork tag, call `flush_event_buffer` with `fork_config: Some(...)`, and assert it returns `FlushResult::Fork(expected_tasks)`.

2. **Skip dedicated test**: the code path mirrors `process_claude_event` exactly — same fork detection, same `run_fork` call. The existing `fork_basic` VCR test validates the fork machinery itself. The new code just routes buffered events through the same machinery.

Going with option 1 — a small unit test gives confidence that the detection works without needing VCR infrastructure.

## Questions

## Review
