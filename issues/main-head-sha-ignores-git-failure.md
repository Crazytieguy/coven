---
priority: P1
state: new
---

# `main_head_sha` doesn't check git command exit status

`src/commands/worker.rs:1058-1069` runs `git rev-parse` but doesn't verify `output.status.success()`:

```rust
fn main_head_sha(worktree_path: &Path) -> Result<String> {
    let main_branch = worktree::main_branch_name(worktree_path)?;
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree_path)
        .args(["rev-parse", &main_branch])
        .output()
        .context("failed to run git rev-parse")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
```

If the git command fails (e.g., corrupt ref, filesystem error, permission issue), `output.stdout` could be empty or contain error text. The function silently returns this garbage value as if it were a valid SHA.

This is used by `wait_for_new_commits` to detect new commits on main by comparing SHAs. A transient git failure would produce a different string than the initial SHA, falsely triggering "new commits detected" and prematurely waking the sleeping worker.

## Fix

Check `output.status.success()` before reading stdout, similar to the `git()` helper in `worktree.rs:76-94`:

```rust
if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    bail!("git rev-parse failed: {}", stderr.trim());
}
```

Alternatively, reuse `worktree::main_branch_name` + a hypothetical `worktree::rev_parse` helper, or call the existing `worktree::git()` helper (though it's private to the worktree module).
