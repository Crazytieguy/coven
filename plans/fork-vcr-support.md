Issue: [P2] Fork: VCR support — `run_fork` spawns sessions directly (bypassing `vcr.call`), and fork detection is gated behind `vcr.is_live()`. Fork behavior is completely untested via VCR.
Status: draft

## Approach

Wrap the entire `run_fork` call as a single VCR entry so fork detection, execution, and reintegration are exercised during VCR tests — without needing to thread VCR through fork's internal child sessions.

### Changes

1. **`src/commands/session_loop.rs`** — Remove the `vcr.is_live()` gate around fork detection (lines 228-238). Always detect fork tags. Then wrap the `run_fork(...)` call in `vcr.call("fork", ...)`:
   - During **recording**: `run_fork` executes normally (spawns real children, renders output, returns the reintegration message). VCR records the result string.
   - During **replay**: VCR returns the recorded result string directly. `run_fork` is never called, so no child processes are spawned.

2. **`src/bin/record_vcr.rs`** — Add a `fork` field to the test case TOML (`[run]` section). Pass it through to the command invocation instead of hardcoding `fork: false`.

3. **New test case `tests/cases/fork_basic.toml`** — A test case where the model's result contains a `<fork>` tag. Since fork children consume API calls and are non-trivial to set up, the recording trigger should use `mode = "exit"` after the reintegration message is sent back and the final result arrives.

4. **Fork args type** — The `vcr.call` needs serializable args. Use the `tasks: Vec<String>` as the args (these are already serializable). The result type is `String` (the reintegration message), also serializable.

### Tradeoff

Fork child rendering (tool calls, completion notices) won't appear in snapshots during replay — the VCR replays only the final reintegration message. This is acceptable because:
- Fork parsing (`parse_fork_tag`) and reintegration (`compose_reintegration_message`) already have unit tests
- The important VCR-level behaviors are: fork tag is detected → fork runs → reintegration message is sent back → session continues
- Fork child rendering can be tested separately if needed later

## Questions

### Should fork detection remain gated during replay, or should we always detect?

During replay, if we always detect fork tags, the VCR `call("fork", ...)` will handle replay by returning the recorded result. This is the simplest approach. But it means the fork tag must be present in the recorded result text, which it will be (it's part of the Claude response that gets recorded via `next_event`).

Alternatively, we could gate fork detection on `!vcr.is_replay()` and only record during live/record modes, but this adds complexity for no benefit since `vcr.call` already handles replay correctly.

Recommendation: always detect (remove the `vcr.is_live()` gate entirely).

Answer:

### Should the `fork` field in test TOML default to false?

Currently fork is hardcoded to false in record_vcr. Adding a TOML field with default false maintains backward compatibility — existing test cases don't need changes.

Answer:

## Review

