Issue: [P0] Add first `coven worker` VCR test case: single worker dispatch → agent → land pipeline
Status: draft

## Approach

### Phase 1: VCR-wrap worker's external operations

Worker.rs calls many external operations that are not VCR-wrapped. All need to be wrapped with `vcr.call()` so they're recorded during recording and stubbed during replay. The VCR context needs to be threaded through to each call site.

**Operations to wrap (in worker.rs):**

1. `worktree::spawn(opts)` → returns `Result<SpawnResult, WorktreeError>`
   - `SpawnResult` already derives `Serialize + Deserialize` → blanket `Recordable` works
   - Args: `SpawnOptions` contains `&Path` refs, not serializable. Add a recorded form (e.g. record branch name + base path strings)
   - Convert `WorktreeError` to `anyhow::Error` before calling `vcr.call`

2. `worktree::sync_to_main(path)` → `Result<(), WorktreeError>`
   - Args: path string. Result: `()`.

3. `worktree::land(path)` → `Result<LandResult, WorktreeError>`
   - `LandResult` already `Serialize + Deserialize`
   - **Important**: `land_or_resolve` matches on `WorktreeError::RebaseConflict(files)`. VCR replays errors as `anyhow!()` string. Two options:
     - (a) Change error matching to parse the anyhow error string
     - (b) Extend VCR to preserve typed errors for `WorktreeError` (custom `Recordable` for `Result<T, WorktreeError>`)
     - (c) Don't VCR-wrap `land` — but then replay needs a real git repo
   - For the first test (happy path, no conflicts), this isn't hit. Defer to follow-up.

4. `worktree::has_unique_commits(path)` → `Result<bool, WorktreeError>`

5. `worktree::clean(path)` → `Result<(), WorktreeError>`

6. `worktree::remove(path)` → `Result<(), WorktreeError>`

7. `worktree::reset_to_main(path)`, `abort_rebase(path)`, `is_rebase_in_progress(path)`, `ff_merge_main(path)` — used in conflict resolution paths, wrap them too for completeness

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

For operations that take paths, record the path as a `String` arg so VCR can assert args match during replay.

### Phase 2: Add `[worker]` support to record_vcr.rs

Extend `TestCase` to support a `[worker]` section alongside existing `[run]` and `[ralph]` sections:

```toml
[worker]
# No prompt needed — dispatch agent determines what to do
claude_args = []
```

In `record_case()`, add a worker branch that:
1. Creates the temp dir with test files (including `.coven/agents/` agent definitions)
2. Inits the git repo
3. Creates a worktree base directory (another temp dir)
4. Runs `commands::worker::worker(WorkerConfig { ... }, &mut io, &vcr, &mut output)`

### Phase 3: Create the test case

**Test case: `worker_basic`**

A minimal single-iteration test: dispatch → agent → land → exit.

**Agent definitions** (in `[files]`):
- `.coven/agents/dispatch.md` — minimal dispatch agent that reads the agent catalog and worker status, outputs `<dispatch>agent: greet\n</dispatch>` on first call
- `.coven/agents/greet.md` — trivial agent that creates a file and commits

**Exit strategy**: After the first land succeeds, the worker loops back to dispatch. The second dispatch session needs to cause exit. Options:
1. Have trigger controller inject Ctrl-C during the second dispatch's Claude session, then Ctrl-D at the follow-up prompt
2. Or: modify the worker to accept a `max_iterations` config (like ralph's `iterations`) — cleaner for testing

I recommend option 2: add `max_iterations: Option<u32>` to `WorkerConfig`, defaulting to `None` (unlimited). This is cleaner than trigger hacks and useful for production too (e.g. "run one iteration then exit").

**Expected snapshot**: Shows worker startup, dispatch output, agent running tools, landing, then exit.

### Phase 4: Test harness updates

Add to `vcr_test.rs`:
- `vcr_test!(worker_basic)` — or a separate macro if worker tests need different setup (e.g. `run_vcr_test` currently only handles run/ralph)

### Implementation order

1. Add `Serialize + Deserialize` to `AgentDef`, `AgentFrontmatter`, `AgentArg`
2. Add custom `Recordable` impl for `DispatchLock`
3. Thread `vcr: &VcrContext` through all worker operations, wrapping each with `vcr.call()`
4. Add `max_iterations` to `WorkerConfig`
5. Add `[worker]` support to `TestCase` and `record_vcr.rs`
6. Add `worker_basic` to `vcr_test.rs`
7. Create test case `.toml` with agent definitions
8. Record: `cargo run --bin record-vcr worker_basic`
9. Iterate on snapshot until it looks correct
10. Run full test suite to verify nothing broke

## Questions

### How should we handle `WorktreeError` matching during VCR replay?

The `land_or_resolve` function matches on `WorktreeError::RebaseConflict(files)` to decide whether to attempt conflict resolution. VCR replays errors as `anyhow!("error string")`, losing the type info.

Options:
- (a) **String matching**: Parse the anyhow error string (fragile but simple)
- (b) **Typed error VCR support**: Extend `vcr.call` to preserve `WorktreeError` variants via serialization (since `WorktreeError` already derives `Serialize + Deserialize`)
- (c) **Don't wrap `land`**: Let it run against a real git repo during replay (requires test setup)
- (d) **Defer**: The first test has no conflicts, so this doesn't apply yet. Handle when adding a conflict test.

I recommend (d) for now — ship the happy-path test first, then tackle conflict testing separately. When we do, (b) is the cleanest solution since `WorktreeError` is already serializable.

Answer:

### Should `max_iterations` be added to `WorkerConfig`?

The worker loop currently runs until interrupted. For testing, we need a clean exit. Options:
- (a) Add `max_iterations: Option<u32>` to `WorkerConfig` — clean, useful beyond testing
- (b) Use trigger controller to inject Ctrl-C/Ctrl-D — hacky, fragile timing
- (c) Have the second dispatch return ProcessExited — depends on trigger timing

Answer:

## Review

