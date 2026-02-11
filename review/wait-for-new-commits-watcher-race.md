---
priority: P2
state: review
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

## Plan

In `src/commands/worker.rs`, reorder `wait_for_new_commits` (lines 1097–1131) so the watcher is active before HEAD is read, closing the TOCTOU gap.

**Changes to `wait_for_new_commits`:**

1. **Move `setup_ref_watcher` before `vcr_main_head_sha`** — swap lines 1105 and 1108 so the watcher is watching before we read the initial HEAD.

2. **Drain + re-check after initial read** — after reading `initial_head`, drain any notifications the watcher may have buffered during setup, then immediately read HEAD again. If it differs from `initial_head`, return `NewCommits` without entering the loop. This catches commits that landed in the window between watcher setup and the initial read.

The resulting function body (lines 1104–1131) becomes:

```rust
let wt_str = worktree_path.display().to_string();

// Set up watcher FIRST so no commits are missed.
let (_watcher, mut rx) = setup_ref_watcher(worktree_path)?;
let initial_head = vcr_main_head_sha(vcr, wt_str.clone()).await?;

// Drain any notifications that fired during setup.
while rx.try_recv().is_ok() {}

// If HEAD already moved while we were setting up, return immediately.
let current = vcr_main_head_sha(vcr, wt_str.clone()).await?;
if current != initial_head {
    renderer.write_raw("New commits detected on main.\r\n");
    return Ok(WaitOutcome::NewCommits);
}

loop {
    tokio::select! {
        _ = rx.recv() => {
            let current = vcr_main_head_sha(vcr, wt_str.clone()).await?;
            if current != initial_head {
                renderer.write_raw("New commits detected on main.\r\n");
                return Ok(WaitOutcome::NewCommits);
            }
        }
        event = vcr.call("next_event", (), async |(): &()| io.next_event().await) => {
            let event = event?;
            if let IoEvent::Terminal(Event::Key(key_event)) = event {
                let action = input.handle_key(&key_event);
                if matches!(action, InputAction::Interrupt | InputAction::EndSession) {
                    return Ok(WaitOutcome::Exited);
                }
            }
        }
    }
}
```

**Why this is correct:** After the watcher is active, any commit that lands will either (a) be caught by the drain+re-check if it arrived during setup, or (b) trigger a watcher notification that feeds the loop. There is no gap.
