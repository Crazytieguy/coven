---
priority: P2
state: new
---

# Race condition in wait_for_new_commits between HEAD read and watcher setup

In `src/commands/worker.rs:1097-1131`, `wait_for_new_commits` has a TOCTOU race between reading the initial HEAD sha and setting up the filesystem watcher:

```rust
let initial_head = vcr_main_head_sha(vcr, wt_str.clone()).await?;  // line 1105
let (_watcher, mut rx) = setup_ref_watcher(worktree_path)?;         // line 1108
```

A commit landing on main between these two lines would:
1. Not trigger the watcher (it wasn't watching yet)
2. Not be caught by the initial HEAD comparison (HEAD was read before the commit)

The worker would block indefinitely until the *next* commit arrives or the user interrupts.

## Where

- `src/commands/worker.rs:1105-1108` — gap between `vcr_main_head_sha` and `setup_ref_watcher`

## Fix

Set up the watcher first, then read the initial HEAD. Any commit that lands after the watcher is active will trigger a notification. Then do one initial comparison to catch commits that landed before the HEAD read:

```rust
let (_watcher, mut rx) = setup_ref_watcher(worktree_path)?;
let initial_head = vcr_main_head_sha(vcr, wt_str.clone()).await?;
// Drain any notifications that arrived during setup
while rx.try_recv().is_ok() {}
// Check if HEAD already changed (covers the setup window)
let current = vcr_main_head_sha(vcr, wt_str.clone()).await?;
if current != initial_head {
    return Ok(WaitOutcome::NewCommits);
}
```
