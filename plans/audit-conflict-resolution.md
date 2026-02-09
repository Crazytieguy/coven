Issue: Audit conflict resolution logic (worker.rs land_or_resolve). Complex nested loop with session resume, rebase retry, and multiple fallback paths — worth a careful review.
Status: draft

## Approach

After thorough review of `land_or_resolve` (worker.rs:331-438), `run_phase_session` (worker.rs:453-525), and `worktree::land` (worktree.rs:259-329), six issues were identified. Here's the revised fix for each, incorporating review feedback:

### 1. Incomplete rebase recovery (worker.rs:416-423)

**Problem**: If Claude doesn't run `git rebase --continue`, the rebase is aborted and all agent work is silently discarded.

**Fix**: Rebase completion is Claude's responsibility, but coven should help Claude succeed rather than silently discard work. When `is_rebase_in_progress` is true after a resolution session:

1. Resume the session with a nudge message like "The rebase is still in progress — please run `git rebase --continue` to complete it."
2. If the rebase is *still* in progress after that second attempt, then abort and count it as a failed resolution attempt (feeding into the retry limit from issue #2).

This gives Claude two chances to complete the rebase before giving up on that attempt. The retry loop (issue #2) provides further opportunities.

Changes:
- `worker.rs`: After detecting rebase-in-progress post-resolution, call `run_phase_session` again with a nudge message instead of immediately aborting. Add `is_rebase_in_progress` check after the nudge; only then abort and retry.

### 2. Bounded retry loop with pause (worker.rs:349-437)

**Problem**: The land-resolve-retry loop has no bound. If main keeps advancing, this worker could loop indefinitely.

**Fix**: Add a retry counter with max 5 attempts. After exhausting retries, instead of discarding work:

1. Abort the current rebase and reset to main.
2. Render a clear message: "Conflict resolution failed after 5 attempts — pausing worker. Press Enter to retry."
3. Block on user input (`stdin().read_line`). When the user presses Enter, reset the counter and re-enter the loop.

This keeps the worker's commits on its branch (not discarded) and lets the user decide when to retry, e.g. after other workers quiesce.

Changes:
- `worker.rs`: Add `let mut attempts = 0;` before the loop. Increment on each conflict. When `attempts >= 5`, abort rebase, reset to main, render pause message, wait for stdin, reset counter.

### 3. Logged `clean()` errors (worker.rs:422, 434)

**Problem**: `let _ = worktree::clean(worktree_path)` silently ignores errors.

**Context**: `clean()` runs `git clean -fd` — it removes untracked, non-ignored files (test artifacts, temp files, editor lockfiles) to prevent stray files from blocking future land attempts. Gitignored files (build artifacts) are preserved.

**Fix**: Replace `let _ =` with a logged warning on failure. Since we're already in a recovery path, don't hard-error — just make the failure visible:

```rust
if let Err(e) = worktree::clean(worktree_path) {
    renderer.write_raw(&format!("Warning: worktree clean failed: {e}\r\n"));
}
```

Changes:
- `worker.rs`: Replace all `let _ = worktree::clean(...)` with the logged pattern above (3-4 call sites).

### 4. Session resume identity (worker.rs:428)

**Problem**: `resume_session_id = session_id;` reassigns to the resolution session's ID.

**Fix**: No code change needed. Since `run_phase_session` resumes the original session via `--resume`, the returned session ID should be the same as the input. The reassignment is redundant but harmless. Add a debug assertion or comment to make this invariant explicit:

```rust
// Resolution resumes the same session — ID should be unchanged.
debug_assert_eq!(resume_session_id.as_deref(), session_id.as_deref());
```

Changes:
- `worker.rs`: Add a `debug_assert_eq!` after `resume_session_id = session_id;` to enforce the same-session invariant, plus a brief comment.

### 5. No durable conflict logging

**Problem**: Conflict files and resolution outcomes are only rendered to the terminal.

**Fix**: No code change. File as a separate low-priority issue for later.

Changes:
- `issues.md`: Add a [P2] issue for durable conflict logging.

### 6. Session-less conflict resolution (worker.rs:378-385)

**Problem**: When `resume_session_id` is `None`, conflicts can't be resolved and work is silently discarded.

**Fix**: This case is rarer than originally thought — the session ID is captured from the Init event at the start of the agent session and persists in coven's memory even if the Claude subprocess dies. `None` only happens if the very first agent run never emitted an Init event (e.g., immediate crash).

Still, we should handle it better: start a fresh (non-resumed) conflict resolution session. Claude won't have the prior context, but it can still read the conflict markers and attempt resolution. This is better than silently discarding work.

Changes:
- `worker.rs`: When `resume_session_id.is_none()`, call `run_phase_session` with `None` as the resume ID (starts a fresh session) instead of aborting. The conflict file list and resolution prompt give Claude enough context to attempt resolution without prior conversation history.

## Summary of code changes

- **worker.rs `land_or_resolve`**:
  - Add rebase-incomplete recovery loop (nudge Claude to run `rebase --continue`)
  - Add retry counter (max 5), pause on exhaustion instead of discarding
  - Replace `let _ = clean()` with logged warnings
  - Add `debug_assert_eq!` for session ID invariant
  - Allow session-less conflict resolution via fresh session
- **issues.md**: Add P2 issue for durable conflict logging

## Questions

None — all questions resolved in previous review.

## Review

