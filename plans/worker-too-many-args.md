Issue: worker.rs too_many_arguments clippy warnings: adding W: Write + renderer parameter pushed 7 functions from 7 to 8 args. Consider grouping related params into a context struct.
Status: draft

## Approach

Introduce a `WorkerCtx<W: Write>` struct that bundles the recurring parameters, and pass `&mut WorkerCtx<W>` instead. This is a purely mechanical refactor — no behavioral changes.

### The struct

```rust
struct WorkerCtx<'a, W: Write> {
    renderer: &'a mut Renderer<W>,
    input: &'a mut InputHandler,
    io: &'a mut Io,
    vcr: &'a VcrContext,
}
```

These four parameters appear together in all 7 internal functions. `worktree_path` and `extra_args` also recur but belong to the calling context (they vary by phase), so they stay as standalone params.

### Resulting signatures

| Function | Before | After |
|---|---|---|
| `worker_loop` | 8 args | 5 (config, worktree_path, branch, ctx, total_cost) |
| `run_dispatch` | 8 args | 5 (worktree_path, branch, extra_args, worker_status, ctx) |
| `run_agent` | 8 args | 5 (prompt, worktree_path, extra_args, ctx, total_cost) |
| `ensure_commits` | 8 args | 5 (worktree_path, agent_session_id, extra_args, ctx, total_cost) |
| `land_or_resolve` | 8 args | 5 (worktree_path, session_id, extra_args, ctx, total_cost) |
| `resolve_conflict` | 8 args | 5 (prompt, worktree_path, sid, extra_args, ctx) |
| `run_phase_session` | 8 args | 5 (prompt, working_dir, extra_args, resume, ctx) |

Every function drops from 8 to 5 args, all well within the 7-arg limit. The helper functions `warn_clean` and `abort_and_reset` that take `(path, renderer)` stay as-is since they only use one field and aren't flagged.

### Steps

1. Define `WorkerCtx<'a, W>` in worker.rs (private, module-internal)
2. Update `worker()` to create the ctx and pass it to `worker_loop`
3. Update each function signature: replace the 4 individual params with `ctx: &mut WorkerCtx<'_, W>`
4. Update call sites within each function body: `renderer` → `ctx.renderer`, `input` → `ctx.input`, `io` → `ctx.io`, `vcr` → `ctx.vcr`
5. Verify clippy passes with 0 `too_many_arguments` warnings in worker.rs

### Not included

`total_cost` is excluded from the struct because it's only used in 4 of 7 functions and is a simple `&mut f64` — adding it would create an unnecessary borrow conflict (the struct would be `&mut` while also needing to pass `total_cost` separately to inner calls).

## Questions

None — the approach is straightforward.

## Review

