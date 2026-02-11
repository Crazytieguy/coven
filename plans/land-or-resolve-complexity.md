Issue: `land_or_resolve` in worker.rs is 123 lines with 4-5 levels of nesting, an infinite loop with complex exit conditions, and multiple concerns (retry logic, conflict resolution, error recovery). Break it into smaller focused functions.
Status: draft

## Approach

The `land_or_resolve` function (lines 527–650) handles three interleaved concerns:

1. **Landing** — calling `worktree::land` and handling success
2. **Retry logic** — counting attempts, pausing on too many failures, distinguishing ff-failure from conflict from other errors
3. **Conflict resolution** — building prompts, running resolution sessions, updating session IDs

### Proposed decomposition

Extract the match arms of the `worktree::land` result into a helper that maps `worktree::land`'s result into a single flat enum:

```rust
enum LandAttempt {
    Landed { branch: String, main_branch: String },
    Conflict(Vec<String>),
    FastForwardRace,
    OtherError(anyhow::Error),
}
```

Then refactor `land_or_resolve` into a simple loop:

```rust
loop {
    match try_land(vcr, wt_str).await? {
        LandAttempt::Landed { .. } => { render success; return Ok(false); }
        LandAttempt::Conflict(files) => {
            // delegate to handle_conflict(...) which manages
            // attempts counter, abort-on-max, prompt building,
            // and calling resolve_conflict
        }
        LandAttempt::FastForwardRace => {
            // delegate to handle_ff_retry(...) which manages
            // ff attempts counter and pause-on-max
        }
        LandAttempt::OtherError(e) => {
            // delegate to handle_land_error(...) which aborts
            // rebase and pauses for user input
        }
    }
}
```

Each handler returns a simple `ControlFlow<bool>` (break = should-exit, continue = retry). This flattens the nesting from 4-5 levels to 2, separates the concerns, and makes the retry/exit logic explicit.

### What stays the same

- `resolve_conflict` and `ResolveOutcome` are already well-factored — no changes needed.
- The overall contract (returns `bool` for should-exit) stays the same.
- Retry constants (`MAX_ATTEMPTS`) stay the same.

## Questions

### Should the ff-failure and conflict-failure attempt counters be unified or separate?

Currently they share a single `attempts` counter, which means 3 ff-failures + 2 conflict-failures triggers the pause. This seems intentional (total disruption budget) but it's ambiguous. Options:

1. **Keep shared** (current behavior) — simple, conservative
2. **Separate counters** — more lenient, but arguably if you're hitting 5 total failures something is wrong regardless

Answer:

## Review

