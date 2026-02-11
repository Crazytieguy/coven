---
priority: P1
state: approved
---

# Replace polling with instant notifications

The worker's `wait_for_new_commits` polls `git rev-parse` every 10 seconds. Replace this with a filesystem watcher (e.g. `notify` crate) on the git refs to get instant wake-up on new commits.

Also audit the codebase for other polling patterns and convert them where possible.

## Plan

### Audit results

Two polling patterns exist:

1. **`wait_for_new_commits`** (`src/commands/worker.rs:1024-1055`) — Polls `git rev-parse main` every 10 seconds via `tokio::select!` with `sleep(10s)`. This is the primary target.

2. **`acquire_dispatch_lock`** (`src/worker_state.rs:232-255`) — Polls `try_lock_exclusive()` every 100ms. This is intentionally designed this way (comment explains: avoids blocking the tokio runtime, prevents deadlocks in VCR recording). 100ms polling on a file lock is already efficient and idiomatic. **Leave as-is.**

### Implementation: replace `wait_for_new_commits` polling with `notify` watcher

#### Step 1: Add `notify` dependency

`cargo add notify` — the `notify` crate provides cross-platform filesystem watching (FSEvents on macOS, inotify on Linux).

#### Step 2: Create a git ref watcher helper

Add a new function in `src/commands/worker.rs` (or a small new module if preferred) that:

1. Resolves `<git-common-dir>` from the worktree path (reuse the same `git rev-parse --git-common-dir` pattern from `worker_state.rs:51-71`)
2. Determines the main branch name via `worktree::main_branch_name()`
3. Sets up a `notify::RecommendedWatcher` watching these paths:
   - `<git-common-dir>/refs/heads/<main-branch>` — loose ref file (updated on most ref changes)
   - `<git-common-dir>/packed-refs` — packed refs file (updated during gc)
4. Bridges watcher events to tokio via a `tokio::sync::mpsc::channel(1)` — the watcher's `EventHandler` sends on the sync side, and the async receiver is awaited in the `tokio::select!`
5. Returns a future/receiver that resolves when any watched path changes

#### Step 3: Rewrite `wait_for_new_commits`

Replace the `sleep(10s)` branch in the `tokio::select!` with the watcher receiver. Keep the overall structure:

```rust
async fn wait_for_new_commits(...) -> Result<WaitOutcome> {
    let initial_head = vcr_main_head_sha(vcr, wt_str.clone()).await?;
    let (watcher, mut rx) = setup_ref_watcher(worktree_path)?;  // new

    loop {
        tokio::select! {
            // Was: sleep(10s). Now: wait for fs notification
            _ = rx.recv() => {
                let current = vcr_main_head_sha(vcr, wt_str.clone()).await?;
                if current != initial_head {
                    renderer.write_raw("New commits detected on main.\r\n");
                    return Ok(WaitOutcome::NewCommits);
                }
                // Spurious notification (e.g. packed-refs rewrite with no actual change) — loop
            }
            event = vcr.call("next_event", ...) => {
                // unchanged: handle Ctrl-C/Ctrl-D
            }
        }
    }
}
```

Key details:
- The watcher variable must be kept alive (not dropped) for the duration of the loop — dropping it stops watching
- The `vcr_main_head_sha` confirmation check stays, both for VCR compatibility and to filter spurious notifications
- No fallback timeout needed: if the watcher fails to fire, the user can always Ctrl-C out, and `notify` is reliable on both macOS and Linux

#### Step 4: VCR compatibility

The watcher setup and notification receipt are **not** wrapped in VCR calls. Rationale:
- During recording, the watcher fires from real filesystem events, which triggers the already-VCR-wrapped `vcr_main_head_sha` call — that's what gets recorded
- During replay, `wait_for_new_commits` isn't called because the VCR replays the outer worker loop deterministically (the dispatch decision and subsequent calls are all recorded)
- The watcher is local plumbing, not an I/O boundary worth recording

If this assumption is wrong (i.e., if VCR replay does enter `wait_for_new_commits`), we'll need to wrap `rx.recv()` in a VCR call that records a unit `()` value. This can be verified by running the existing worker VCR tests after the change.

#### Step 5: Test

- Run `cargo test` to verify existing tests pass
- Run `cargo clippy` and `cargo fmt`
- Manually verify with `coven worker` that the watcher picks up new commits instantly

## Answered Questions

1. **VCR assumption**: Implementer to decide — check by running the tests whether `wait_for_new_commits` is reached during replay, and wrap in VCR if needed.

2. **Fallback timeout**: No fallback needed. The watcher is reliable enough on its own.
