Issue: [P0] Add first `coven worker` VCR test case: single worker dispatch → agent → land pipeline
Status: draft

## Approach

### Phase 1: Typed error support in VCR

Currently, `vcr.call()` always stringifies errors (`Err(format!("{e:#}"))` during recording, `anyhow!("{msg}")` during replay). This loses typed error information — a problem for worker tests since `land_or_resolve` matches on `WorktreeError::RebaseConflict` and `WorktreeError::FastForwardFailed`.

Add typed error support to the VCR system:

1. Add a new trait (e.g. `RecordableError`) for error types that derive `Serialize + Deserialize`. Implement it for `WorktreeError`.
2. Add a `vcr.call_typed_err()` variant (or extend `vcr.call()`) that serializes the full error enum variant during recording and deserializes it back during replay, preserving match-ability.
3. For callers that don't need typed errors, the existing string-based behavior remains the default.

This is a general VCR improvement — any serializable error type can opt in.

### Phase 2: VCR-wrap worker's external operations

Worker.rs calls many external operations that are not VCR-wrapped. All need to be wrapped with `vcr.call()` so they're recorded during recording and stubbed during replay. The VCR context needs to be threaded through to each call site.

**Operations to wrap (in worker.rs):**

1. `worktree::spawn(opts)` → returns `Result<SpawnResult, WorktreeError>`
   - `SpawnResult` already derives `Serialize + Deserialize` → blanket `Recordable` works
   - Args: `SpawnOptions` contains `&Path` refs, not serializable. Add a recorded form (e.g. record branch name + base path strings)
   - Use typed error VCR for `WorktreeError`

2. `worktree::sync_to_main(path)` → `Result<(), WorktreeError>`
   - Args: path string. Result: `()`.

3. `worktree::land(path)` → `Result<LandResult, WorktreeError>`
   - `LandResult` already `Serialize + Deserialize`
   - Use typed error VCR — `land_or_resolve` matches on `RebaseConflict` and `FastForwardFailed`

4. `worktree::has_unique_commits(path)` → `Result<bool, WorktreeError>`

5. `worktree::clean(path)` → `Result<(), WorktreeError>`

6. `worktree::remove(path)` → `Result<(), WorktreeError>`

7. `worktree::reset_to_main(path)`, `abort_rebase(path)`, `is_rebase_in_progress(path)`, `ff_merge_main(path)` — used in conflict resolution paths, wrap for completeness

8. `worker_state::register(path, branch)` → `Result<()>`

9. `worker_state::update(path, branch, agent, args)` → `Result<()>`

10. `worker_state::deregister(path)` → `()` (infallible, just log warning)

11. `worker_state::acquire_dispatch_lock(path)` → `Result<DispatchLock>`
    - `DispatchLock` holds a `File` — needs custom `Recordable` impl: record as `()`, reconstruct as a dummy (e.g. from `/dev/null`)

12. `worker_state::read_all(path)` → `Result<Vec<WorkerState>>`
    - `WorkerState` already `Serialize + Deserialize`

13. `agents::load_agents(dir)` → `Result<Vec<AgentDef>>`
    - `AgentDef` is not `Serialize + Deserialize` — add derives (and to `AgentFrontmatter`, `AgentArg`)

**Pattern for wrapping:** Each call becomes:
```rust
let result = vcr.call("worktree::sync_to_main", path_str, async |p: &String| {
    worktree::sync_to_main(Path::new(p)).map_err(Into::into)
}).await?;
```

For operations with typed errors (worktree ops), use the new typed error variant instead.

### Phase 3: Add `[worker]` support to record_vcr.rs

Extend `TestCase` to support a `[worker]` section alongside existing `[run]` and `[ralph]` sections:

```toml
[worker]
claude_args = []
```

In `record_case()`, add a worker branch that:
1. Uses the existing temp dir + git repo setup (already handled by `setup_test_dir()`)
2. Passes a sensible worktree base dir to `WorkerConfig` (e.g. a sibling temp dir — the worker creates its own worktrees under this)
3. Runs `commands::worker::worker(WorkerConfig { ... }, &mut io, &vcr, &mut output)`

For test exit: when dispatch outputs `sleep`, the worker calls `wait_for_new_commits()` which accepts user input. The test infrastructure should inject Ctrl-D (via trigger or auto-exit) when sleep is detected, causing `WaitOutcome::Exited` and a clean return.

### Phase 4: Create the test case

**Test case: `worker_basic`**

A minimal single-iteration test: dispatch → agent → land → sleep → exit.

**Agent definitions** (in `[files]`):
- `.coven/agents/dispatch.md` — a dispatch agent prompt that describes the task context (available agents, worker status) and asks the model to decide what to do. The prompt must NOT mention the `<dispatch>` tag — the system prompt already instructs the model on the output format. This tests that the system prompt works correctly.
- `.coven/agents/greet.md` — trivial agent that creates a file and commits

**Test flow:**
1. Worker starts, syncs worktree, acquires dispatch lock
2. Dispatch agent runs, examines filesystem/git state, sees work to do, dispatches `greet` agent
3. `greet` agent creates a file and commits
4. Worker lands the commit (happy path, no conflicts)
5. Worker loops back to dispatch
6. Dispatch agent runs again, examines filesystem/git state, sees work is already done, outputs `sleep`
7. Worker enters sleep mode, test infrastructure injects Ctrl-D, worker exits cleanly

The dispatch agent decides based on the filesystem/git state — this is the intended production use case and important to test end-to-end.

**Expected snapshot**: Shows worker startup, dispatch output, agent running tools, landing, dispatch sleep, then exit.

### Phase 5: Test harness updates

Add to `vcr_test.rs`:
- `vcr_test!(worker_basic)` — or a separate macro if worker tests need different setup (e.g. `run_vcr_test` currently only handles run/ralph)

### Implementation order

1. Add typed error support to VCR (`RecordableError` trait + `call_typed_err` or similar)
2. Add `Serialize + Deserialize` to `AgentDef`, `AgentFrontmatter`, `AgentArg`
3. Add custom `Recordable` impl for `DispatchLock`
4. Thread `vcr: &VcrContext` through all worker operations, wrapping each with `vcr.call()` (using typed errors for worktree ops)
5. Add `[worker]` support to `TestCase` and `record_vcr.rs`
6. Create test case `.toml` with agent definitions
7. Add `worker_basic` to `vcr_test.rs`
8. Record: `cargo run --bin record-vcr worker_basic`
9. Iterate on snapshot until it looks correct
10. Run full test suite to verify nothing broke

## Questions

None — previous questions resolved via review feedback.

## Review

