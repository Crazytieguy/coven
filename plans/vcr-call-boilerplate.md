Issue: [P2] VCR call boilerplate in worker.rs: ~10 instances of `vcr.call("name", args, async |a| { op(a) }).await?` with similar shapes. Extract helper functions to reduce repetition.
Status: draft

## Approach

Add `call_path` and `call_path_typed_err` convenience methods to `VcrContext` that handle the ubiquitous `String` → `Path::new` conversion pattern.

### New methods on `VcrContext` (in `src/vcr.rs`)

```rust
/// Convenience for VCR calls that take a path string and call a sync function with `&Path`.
pub async fn call_path<T>(
    &self,
    label: &str,
    path_str: String,
    f: impl FnOnce(&Path) -> Result<T>,
) -> Result<T>
where
    T: Recordable,
{
    self.call(label, path_str, async |p: &String| f(Path::new(p))).await
}

/// Like `call_path` but preserves typed errors.
pub async fn call_path_typed_err<T, E>(
    &self,
    label: &str,
    path_str: String,
    f: impl FnOnce(&Path) -> std::result::Result<T, E>,
) -> Result<std::result::Result<T, E>>
where
    T: Recordable,
    E: RecordableError,
{
    self.call_typed_err(label, path_str, async |p: &String| f(Path::new(p))).await
}
```

### Changes in `src/commands/worker.rs`

**Simplify inline calls** — Replace verbose closures with direct function references:

```rust
// Before:
ctx.vcr.call_typed_err("worktree::sync_to_main", wt_str.clone(), async |p: &String| {
    worktree::sync_to_main(Path::new(p))
}).await?

// After:
ctx.vcr.call_path_typed_err("worktree::sync_to_main", wt_str.clone(), worktree::sync_to_main).await?
```

Calls that benefit (11 sites):
- `worktree::sync_to_main`, `worktree::land`, `worktree::remove`, `worktree::clean`, `worktree::reset_to_main` (all `call_path_typed_err`)
- `worker_state::acquire_dispatch_lock`, `worker_state::read_all`, `agents::load_agents` (all `call_path`)
- `worker_state::deregister` (`call_path`, wraps the non-Result fn)
- Existing helpers `vcr_abort_rebase`, `vcr_has_unique_commits`, `vcr_is_rebase_in_progress`, `vcr_main_head_sha` all become one-liners

**Calls NOT changed** (intentionally — their arg shapes differ):
- `worker_paths` (unit arg, complex closure)
- `worktree::spawn` (SpawnArgs, not a path)
- `process_id` (unit arg)
- `worker_state::register` (tuple arg)
- `worker_state::update` (WorkerUpdateArgs)
- `next_event` (unit arg, async I/O)

### `worker_state::deregister` wrapping

`deregister` returns `()` not `Result`, so the `call_path` closure would be:
```rust
vcr.call_path("worker_state::deregister", wt_str.clone(), |p| {
    worker_state::deregister(p);
    Ok(())
}).await?
```
This can't use a bare function reference but still benefits from the Path conversion.

## Questions

### Should `call_path` live on `VcrContext` or as free functions in worker.rs?

`call_path` is generic (not domain-specific) and `Path` is a fundamental Rust type, so `VcrContext` seems appropriate. However, it's currently only used in worker.rs. If it feels too worker-specific for the VCR module, it could be a local helper or trait extension.

Answer:

## Review

