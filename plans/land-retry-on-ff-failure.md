Issue: [P1] land_or_resolve discards agent work on FastForwardFailed instead of retrying
Status: draft

## Approach

In `src/commands/worker.rs` `land_or_resolve()`, the `Err(e)` match arm (line 476) catches all non-conflict `land()` errors and responds by resetting to main — permanently discarding the agent's committed work.

The `FastForwardFailed` case is a race condition: another worker advanced main between the successful rebase and the ff-merge. This is fully retryable — calling `land()` again would re-rebase the commits onto the new main and retry the ff-merge.

### Change

In the error match arm of `land_or_resolve`, handle `FastForwardFailed` separately:

```rust
Err(worktree::WorktreeError::FastForwardFailed) => {
    attempts += 1;
    if attempts > MAX_ATTEMPTS {
        renderer.write_raw("Fast-forward failed after max attempts — resetting to main.\r\n");
        worktree::reset_to_main(worktree_path)?;
        warn_clean(worktree_path, renderer);
        return Ok(false);
    }
    renderer.write_raw("Main advanced during land — retrying...\r\n");
    continue;
}
```

This reuses the existing `attempts` counter and `MAX_ATTEMPTS` limit, so a persistent race still terminates. The existing catch-all `Err(e)` arm remains for truly unrecoverable errors (DirtyWorkingTree, DetachedHead, GitCommand, etc.).

### Files changed

- `src/commands/worker.rs`: Add `FastForwardFailed` match arm before the catch-all `Err(e)`.

## Questions

### Should we also tag the branch before resetting on unrecoverable errors?

For the remaining unrecoverable errors (GitCommand, DirtyWorkingTree, etc.), the agent's work is still permanently lost. We could `git tag coven/lost/<branch>/<timestamp>` before `reset_to_main` to preserve the commits in the reflog/tags for manual recovery.

Pros: prevents permanent work loss in any error path.
Cons: accumulates tags that need cleanup; these errors are rare and usually indicate something fundamentally wrong.

Answer:

## Review
