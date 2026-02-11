---
priority: P1
state: review
---

# Audit agent should be allowed to find nothing and sleep

The audit agent currently always proposes issues. If none of its findings are genuine improvements, it creates low-value issues anyway, which can lead to an infinite audit loop — audit finds nothing real, creates weak issues, those get rejected or resolved, audit runs again, repeat.

The audit agent should have the affordance to sleep (like the dispatch agent) when it doesn't find any issues worth adding. It's fine to audit and conclude "nothing to report."

## Plan

Two changes — one to the prompt, one to the worker loop:

### 1. Update audit agent prompt (`.coven/agents/audit.md`)

Add a section after "Guidelines" telling the agent it's okay to find nothing:

```markdown
## Finding Nothing

If you review the codebase and don't find any genuine issues worth reporting, that's a valid outcome. Simply don't create any issue files and don't make any commits. Don't manufacture low-value issues just to have output — no findings is better than weak findings.
```

### 2. Sleep after no-commit agents (`src/commands/worker.rs`)

Currently, when an agent produces no commits, `run_agent` prints "Agent produced no commits — skipping land" and returns `Ok(false)`, which immediately loops back to dispatch. If dispatch has no other work, it re-dispatches audit, creating a tight loop.

**Change `run_agent` return type** from `Result<bool>` (should_exit) to `Result<AgentOutcome>`:

```rust
enum AgentOutcome {
    Landed,
    NoCommits,
    Exited,
}
```

**Update `run_agent`** to return `AgentOutcome::NoCommits` in the `CommitCheck::NoCommits` arm, `AgentOutcome::Exited` for exit cases, and `AgentOutcome::Landed` after successful land.

**Update the worker loop** in the `DispatchDecision::RunAgent` arm to match on the outcome:

```rust
match run_agent(...).await? {
    AgentOutcome::Exited => return Ok(()),
    AgentOutcome::NoCommits => {
        // Agent found nothing to do — wait for new commits before re-dispatching
        ctx.renderer.write_raw("Sleeping until new commits...\r\n");
        ctx.renderer.set_title(&format!("cv sleeping — {branch}"));
        ctx.io.clear_event_channel();
        let wait = wait_for_new_commits(worktree_path, ctx.renderer, ctx.input, ctx.io, ctx.vcr);
        if matches!(wait.await?, WaitOutcome::Exited) {
            return Ok(());
        }
    }
    AgentOutcome::Landed => {}
}
```

This reuses the existing `wait_for_new_commits` infrastructure — the worker sleeps until another worker pushes to main (or the user interrupts), then wakes up and re-dispatches normally.
