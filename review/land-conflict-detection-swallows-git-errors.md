---
priority: P1
state: review
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

## Plan

In `src/worktree.rs`, `land()` function (~line 311-312):

1. **Replace `.unwrap_or_default()` with explicit error handling.** Change:

   ```rust
   let conflicts =
       git(worktree_path, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default();
   ```

   to:

   ```rust
   let conflicts = match git(worktree_path, &["diff", "--name-only", "--diff-filter=U"]) {
       Ok(output) => output,
       Err(diff_err) => {
           let stderr = String::from_utf8_lossy(&rebase_output.stderr);
           return Err(WorktreeError::GitCommand(format!(
               "rebase failed: {} (and failed to list conflicts: {diff_err})",
               stderr.trim()
           )));
       }
   };
   ```

   This way:
   - If `git diff` succeeds, we continue to the existing conflict-file parsing logic (unchanged).
   - If `git diff` itself fails, we return a `GitCommand` error with context about **both** failures — the original rebase failure and the diff failure — so the cause is diagnosable.

2. **No other changes needed.** The rest of the `land()` logic (empty-files → `GitCommand`, non-empty → `RebaseConflict`) and the worker dispatch in `src/commands/worker.rs:546` already handle these variants correctly.
