Issue: Audit conflict resolution logic (worker.rs land_or_resolve). Complex nested loop with session resume, rebase retry, and multiple fallback paths — worth a careful review.
Status: draft

## Approach

After thorough review of `land_or_resolve` (worker.rs:331-438), `run_phase_session` (worker.rs:453-525), and `worktree::land` (worktree.rs:259-329), I identified six issues ranging from silent data loss to unbounded retries. Here's the proposed fix for each:

### 1. Silent work loss on incomplete resolution (worker.rs:416-423)

**Problem**: If Claude doesn't run `git rebase --continue`, the rebase is aborted and all agent work is silently discarded. The worker just continues to the next iteration with no error or warning.

**Fix**: Add a visible warning line before aborting, e.g. "Agent work discarded — conflict resolution did not complete the rebase." This makes the failure visible in the terminal output without changing control flow.

### 2. Unbounded retry loop (worker.rs:349-437)

**Problem**: The land-resolve-retry loop has no bound. If main keeps advancing (other workers landing), this worker could loop indefinitely retrying rebase + resolution.

**Fix**: Add a retry counter (e.g., max 3 attempts). After exhausting retries, abort the rebase, reset to main, log a clear message ("Conflict resolution failed after N attempts, discarding work"), and return.

### 3. Ignored `clean()` errors (worker.rs:422, 434)

**Problem**: `let _ = worktree::clean(worktree_path)` silently ignores errors. If clean fails, the worktree may be left in a dirty state for the next iteration.

**Fix**: Log a warning if clean fails (don't hard-error, since we're already in a failure path). Use `if let Err(e) = worktree::clean(...) { renderer.write_raw(&format!("Warning: worktree clean failed: {e}\r\n")); }`.

### 4. Session resume fragility (worker.rs:428)

**Problem**: `resume_session_id = session_id;` on line 428 reassigns to the resolution session's ID. If the resolution session was truncated or produced minimal output, the next round of conflict resolution has poor context.

**Fix**: No code change — this is working as designed. The resolution session has the most recent context about what was attempted. Document this choice with a brief comment explaining why we chain resolution sessions rather than always resuming from the original.

### 5. No durable conflict logging

**Problem**: Conflict files and resolution outcomes are only rendered to the terminal. There's no way to debug failed resolutions after the fact.

**Fix**: No code change for now — this is a nice-to-have. File as a separate low-priority issue if desired. The terminal output is captured by coven's display layer already.

### 6. Session-less conflict resolution fallback (worker.rs:378-385)

**Problem**: If there's no session ID (e.g., the agent process was killed), conflicts can't be resolved and work is silently discarded.

**Fix**: The current behavior (abort + reset + continue) is reasonable. Add a warning message: "No session available for conflict resolution — discarding work."

## Summary of code changes

- **worker.rs `land_or_resolve`**: Add retry bound (max 3), improve warning messages on silent-discard paths (3 locations), add a comment explaining session chaining. Replace `let _ = clean()` with logged warning.
- No changes to worktree.rs or runner.rs.

## Questions

### Retry limit value

A max of 3 retry attempts seems reasonable — enough to handle one concurrent landing, not so many that it loops forever. Is 3 the right number, or would you prefer a different bound?

Answer:

## Review

