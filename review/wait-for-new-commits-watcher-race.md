---
priority: P2
state: review
---

# Race condition in wait_for_new_commits between HEAD read and watcher setup

In `src/commands/worker.rs:1097-1131`, `wait_for_new_commits` has a TOCTOU race between reading the initial HEAD sha and setting up the filesystem watcher:

```rust
let initial_head = vcr_main_head_sha(vcr, wt_str.clone()).await?;  // line 1114
let (_watcher, mut rx) = setup_ref_watcher(worktree_path)?;         // line 1117
```

A commit landing on main between these two lines would:
1. Not trigger the watcher (it wasn't watching yet)
2. Not be caught by the initial HEAD comparison (HEAD was read before the commit)

The worker would block indefinitely until the *next* commit arrives or the user interrupts.

## Where

- `src/commands/worker.rs:1114-1117` — gap between `vcr_main_head_sha` and `setup_ref_watcher`

## Plan

**Swap lines 1114 and 1117** — move `setup_ref_watcher` before `vcr_main_head_sha` so the watcher is active before HEAD is read.

The change in `wait_for_new_commits` (lines 1113–1117) becomes:

```rust
let wt_str = worktree_path.display().to_string();

// Set up watcher FIRST so no commits are missed.
let (_watcher, mut rx) = setup_ref_watcher(worktree_path)?;
let initial_head = vcr_main_head_sha(vcr, wt_str.clone()).await?;
```

The loop body stays unchanged.

**Why this is sufficient (no drain+re-check needed):**

The previous plan included a drain+re-check after the initial HEAD read, which added an extra `vcr_main_head_sha` call that broke VCR recording. This is unnecessary because:

- **Commit between watcher setup and HEAD read:** The watcher fires (notification buffered). `initial_head` captures the new commit. When the loop processes the buffered notification, `current == initial_head` — a harmless spurious wakeup. The function correctly waits for the *next* commit. No gap.
- **Commit after HEAD read:** Watcher fires, loop detects `current != initial_head`. Correct.
- **Commit before watcher setup:** Same baseline as before — part of the state the worker has already processed.

The only scenario the original code got wrong was a commit between HEAD read and watcher setup (watcher not yet watching, HEAD already stale). Swapping the order eliminates this gap entirely.

**Why no VCR re-recording is needed:**

`setup_ref_watcher` is a plain synchronous function — not a VCR call. Moving it before `vcr_main_head_sha` doesn't change the VCR call sequence at all. The recorded fixtures remain valid.

**Verification:** `cargo test` (no re-recording needed).
