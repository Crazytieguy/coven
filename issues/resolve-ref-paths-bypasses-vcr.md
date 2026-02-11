---
priority: P2
state: new
---

# `resolve_ref_paths` runs git commands outside VCR

`src/commands/worker.rs:1067-1093` calls `worktree::main_branch_name()` and `std::process::Command::new("git")` directly, bypassing VCR recording:

```rust
fn resolve_ref_paths(worktree_path: &Path) -> Option<RefPaths> {
    let main_branch = worktree::main_branch_name(worktree_path).ok()?;
    let output = std::process::Command::new("git")
        .arg("-C").arg(worktree_path)
        .args(["rev-parse", "--git-common-dir"])
        .output().ok()?;
    // ...
}
```

This violates the project convention that all external I/O must go through `vcr.call()`.

## Impact

Low — the function is best-effort with `Option<RefPaths>` return and graceful fallback. During VCR replay, the git commands fail (worktree doesn't exist), `resolve_ref_paths` returns `None`, the watcher watches nothing, and the loop relies entirely on VCR-replayed `next_event` calls. Behavior is correct in all modes.

## Fix

Wrap the path resolution in a `vcr.call()`. This would require either:
1. Making `setup_ref_watcher` async and accepting `&VcrContext`, or
2. Pre-resolving the paths via VCR before calling `setup_ref_watcher`

Option 2 is simpler: resolve the ref paths through VCR at the start of `wait_for_new_commits` and pass them into `setup_ref_watcher`.
