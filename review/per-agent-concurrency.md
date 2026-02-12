---
priority: P1
state: review
---

# Per-agent concurrency semaphores

## Motivation

Split from the generic agent loop issue. The current system uses a single exclusive dispatch lock. The generic agent loop replaces this with per-agent concurrency control, but that's a clean add-on that can be done after the core loop refactor lands.

## Design

### `max_concurrency` frontmatter field

Agent frontmatter gains an optional `max_concurrency` field:

```yaml
---
description: "Route work to agents based on issue state"
max_concurrency: 1
args: ...
---
```

Default if unspecified: unlimited (no concurrency restriction).

### Counted file lock semaphores

Before running an agent, the worker acquires a semaphore permit for that agent type. Implementation: counted file locks in `<git-common-dir>/coven/semaphores/`.

For an agent with `max_concurrency: N`:
- Semaphore files: `<git-common-dir>/coven/semaphores/<agent>.0.lock` through `<agent>.<N-1>.lock`
- Worker tries `try_lock_exclusive` on each file in sequence
- If all N are locked, async-retry with sleep (like current dispatch lock)
- Lock released on drop (RAII)

### Changes

1. Add `max_concurrency: Option<u32>` to `AgentFrontmatter` in `src/agents.rs`
2. New module `src/semaphore.rs` implementing counted file lock semaphores
3. In `worker_loop`: before running any agent, acquire semaphore if `max_concurrency` is set
4. Remove the temporary dispatch lock (from the generic loop issue) — replace with semaphore acquisition for the entry agent
5. Update dispatch agent template: add `max_concurrency: 1`
6. Update `coven init` templates accordingly

### Depends on

- Generic agent loop (issues/worker-generic-agent-loop.md)

## Plan

### Step 1: Add `max_concurrency` to `AgentFrontmatter`

In `src/agents.rs`, add an optional field to the `AgentFrontmatter` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFrontmatter {
    pub description: String,
    #[serde(default)]
    pub args: Vec<AgentArg>,
    pub max_concurrency: Option<u32>,
}
```

The `Option<u32>` type means it's omitted from YAML by default (unlimited concurrency). `serde` handles `Option` fields as absent-means-`None` by default, so no `#[serde(default)]` needed.

Add a unit test: parse an agent file with `max_concurrency: 2` in frontmatter, verify it deserializes to `Some(2)`. Verify existing tests still pass (existing agents without the field get `None`).

### Step 2: New module `src/semaphore.rs`

Create `src/semaphore.rs` and register it in `src/lib.rs`.

**`SemaphorePermit` struct** (RAII guard):

```rust
pub struct SemaphorePermit {
    _file: File,
}
```

Holds the locked file. Lock released on drop when `File` is dropped (fs2 behavior). Same pattern as `DispatchLock` in `worker_state.rs`.

**`Recordable` impl** for VCR:

```rust
impl crate::vcr::Recordable for SemaphorePermit {
    type Recorded = ();
    fn to_recorded(&self) -> Result<()> { Ok(()) }
    fn from_recorded((): ()) -> Result<Self> {
        let file = File::open("/dev/null")?;
        Ok(SemaphorePermit { _file: file })
    }
}
```

Same pattern as `DispatchLock`'s `Recordable` impl — during replay, create a dummy permit that doesn't actually hold any lock.

**`acquire` function**:

```rust
pub async fn acquire(repo_path: &Path, agent_name: &str, max_concurrency: u32) -> Result<SemaphorePermit>
```

Implementation:
1. Resolve `<git-common-dir>/coven/semaphores/` using the existing `coven_dir()` helper (needs to be made `pub(crate)` — it's currently private in `worker_state.rs`).
2. `create_dir_all` the semaphores directory.
3. Loop: for `i` in `0..max_concurrency`, try `OpenOptions::new().create(true).truncate(false).write(true).open(path)` then `try_lock_exclusive()` on `<agent_name>.<i>.lock`.
4. If any lock succeeds, return `SemaphorePermit { _file: file }`.
5. If all slots are locked (`WouldBlock`), `tokio::time::sleep(100ms)` and retry from step 3.
6. Propagate non-`WouldBlock` errors immediately.

Like the dispatch lock, this retries forever — an automatic timeout could cause two workers to run the same exclusive agent simultaneously.

**Shared helper**: Extract `coven_dir()` from `worker_state.rs` into a `pub(crate)` function accessible to both modules. Options:
- Move it to a small `src/git_common.rs` utility module, or
- Move it to `worker_state.rs` as `pub(crate)` and import from there.

The simpler option: make `coven_dir` `pub(crate)` in `worker_state.rs`. It's already there and tested implicitly. No need for a new module just for one helper.

**Unit tests:**
- Acquire with `max_concurrency: 1` — succeeds, creates lock file.
- Acquire with `max_concurrency: 2` — two permits can be held simultaneously.
- Acquire with `max_concurrency: 1` when already held — blocks (test with a short timeout to verify it doesn't return immediately; or hold one permit and verify a `try` variant returns `None`).

### Step 3: Integrate into worker loop

In the worker loop (after the generic agent loop refactor), the inner loop currently has:

```
if is_entry:
    acquire dispatch lock
```

Replace this with generic semaphore acquisition:

```
if agent_def.frontmatter.max_concurrency is Some(n):
    permit = vcr.call("semaphore::acquire", ..., semaphore::acquire(repo_path, agent_name, n))
    // permit held for duration of agent session
```

The semaphore is acquired **before any agent** that declares `max_concurrency`, not just the entry agent. The permit is held for the duration of the agent session and dropped afterward (when the variable goes out of scope or is reassigned on the next loop iteration).

VCR wrapping: wrap the `semaphore::acquire` call in `vcr.call()` using the same pattern as `worker_state::acquire_dispatch_lock`. The VCR key should include the agent name for clarity, e.g. `"semaphore::acquire::{agent_name}"`.

The `is_entry` check and dispatch lock acquisition are removed entirely.

### Step 4: Remove the dispatch lock

- Delete `DispatchLock` struct and its `Recordable` impl from `worker_state.rs`.
- Delete `acquire_dispatch_lock()` from `worker_state.rs`.
- Delete the `dispatch_lock_acquire_release` test.
- Remove the dispatch lock VCR call from the worker loop.
- Remove `dispatch.lock` reference from the module doc comment.

The dispatch lock's behavior (exclusive access for dispatch) is now provided by the dispatch agent's `max_concurrency: 1`.

### Step 5: Update dispatch agent template

In `.coven/agents/dispatch.md`, add `max_concurrency: 1` to the frontmatter:

```yaml
---
description: "Route work to agents based on issue state"
max_concurrency: 1
args:
  ...
---
```

This replaces the hardcoded dispatch lock — the semaphore system enforces that only one dispatch runs at a time.

The embedded template in `src/commands/init.rs` picks this up automatically via `include_str!`.

### Step 6: Re-record VCR tests

Re-record orchestration tests since the dispatch lock VCR calls change to semaphore VCR calls:

```bash
cargo run --bin record-vcr worker_basic
cargo run --bin record-vcr concurrent_workers
cargo run --bin record-vcr landing_conflict
cargo run --bin record-vcr needs_replan
cargo run --bin record-vcr priority_dispatch
```

The `concurrent_workers` test is the most important — it should now show semaphore acquisition/retry instead of dispatch lock acquisition.

Run `cargo insta review` to verify snapshot diffs look correct. Run `cargo test` to confirm everything passes.
