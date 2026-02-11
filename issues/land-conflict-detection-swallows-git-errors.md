---
priority: P1
state: new
---

# `land()` conflict detection swallows git errors

`src/worktree.rs:311-312` — when rebase fails, the conflict file list is obtained via:

```rust
let conflicts =
    git(worktree_path, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
```

If the `git diff` command itself fails (corrupted index, disk full, permission error), `.unwrap_or_default()` silently returns an empty string. This causes `conflict_files` to be empty, and `land()` falls through to the generic `GitCommand("rebase failed: ...")` error instead of `RebaseConflict(files)`.

**Impact:** The worker's conflict resolution logic (`src/commands/worker.rs`) dispatches differently based on `RebaseConflict` vs `GitCommand` errors. A swallowed git error here causes the worker to skip conflict resolution and go straight to the `needs-replan` state, losing the chance to resolve the conflict automatically.

**Fix:** Propagate the error with `?` instead of `.unwrap_or_default()`. If the diff command fails, return `GitCommand` with context about both the rebase failure and the diff failure.
