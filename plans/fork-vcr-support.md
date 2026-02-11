Issue: [P1] Fork: VCR support — `run_fork` spawns sessions directly (bypassing `vcr.call`), and fork detection is gated behind `vcr.is_live()`. Fork behavior is completely untested via VCR.
Status: draft

## Approach

Thread the VCR context through fork child sessions so each child's spawn and events are individually recorded and replayed. This enables full fork rendering in VCR tests (tool calls, completion notices, errors) — not just the final reintegration string.

### Changes

1. **`src/commands/session_loop.rs`** — Remove the `vcr.is_live()` gate around fork detection (always detect fork tags). Pass `vcr` to `run_fork()`.

2. **`src/fork.rs`** — Add `vcr: &VcrContext` parameter to `run_fork()`:
   - Wrap each child spawn in `vcr.call("fork_spawn", child_config, ...)`. During replay, spawns return stubs and the captured `child_tx` is dropped (multiplexer tasks exit cleanly via `child_rx.recv()` returning `None`).
   - Wrap each merged channel read in `vcr.call("fork_event", (), ...)`, recording `Option<(usize, AppEvent)>` values. During replay, events are returned from VCR without reading from the channel.
   - Fork child events (tool calls, completions, errors) are individually recorded and appear in test snapshots during replay.

3. **`src/bin/record_vcr.rs`** — Add an optional `fork` field (default `false`) to the test TOML. Pass it through to the command config instead of hardcoding `fork: false`.

4. **New test case `tests/cases/fork_basic.toml`** — A test where the model emits a `<fork>` tag. Use `mode = "exit"` trigger so recording ends after the reintegration cycle completes.

### Design note

Fork children run in parallel, producing an interleaved event stream on a merged channel. VCR naturally serializes this interleaving during recording and replays it deterministically. Each recorded event includes the child index, so rendering knows which child produced which event.

## Questions

## Review
