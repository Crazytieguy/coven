Issue: It would be nice if issues could be "claimed" in a concurrency-safe way, so that multiple agents can work with the same issue list
Status: draft

## Approach

Add a lightweight file-based claiming mechanism to the workflow. Before an agent starts working on an issue, it creates a claim file that signals "this issue is taken."

### Mechanism

1. Add a `claims/` directory (gitignored — claims are ephemeral local state, not shared via git).
2. Update workflow.md to instruct agents: before picking an issue to plan or implement, attempt to create a claim file `claims/<issue-kebab>.claim` using `set -o noclobber` + redirect (`set -C && echo $$ > claims/foo.claim`). If the file already exists, the create fails atomically and the agent should pick a different issue.
3. When an issue is resolved (removed from issues.md), the resolving agent deletes the corresponding claim file.
4. Stale claims: if a claim file is older than some threshold (e.g., 1 hour), agents may delete it and re-claim. This handles crashed/abandoned agents.

### Changes

- **`.gitignore`**: Add `claims/` entry.
- **`workflow.md`**: Add a "Claiming issues" section between current priorities and writing-plans sections, documenting the protocol.
- **No code changes** — this is purely a workflow/convention change.

### Why file-based?

- Shell `noclobber` (`set -C`) provides true atomic file creation at the OS level — if two agents race, exactly one succeeds and the other gets an error.
- No dependencies, no external services, no git race conditions.
- Works locally (which is the current deployment model).

## Questions

### Should claims be git-tracked or gitignored?

Git-tracked claims would enable distributed concurrency (multiple machines), but add noise to the repo history and require push/pull synchronization (which reintroduces race conditions). Gitignored claims are simpler and sufficient for local multi-agent concurrency.

Answer:

### Staleness threshold

1 hour seems reasonable for detecting abandoned claims, but the right value depends on how long a typical plan/implementation takes. Should this be configurable, or is a fixed 1-hour threshold fine?

Answer:

### Claim granularity

Should claiming apply to:
- (a) Planning only — claim when starting to write a plan, release after committing the plan
- (b) Full lifecycle — claim when starting any work on an issue, release when the issue is resolved
- (c) Both, with different claim types (e.g., `.plan-claim` vs `.work-claim`)

Option (b) is simplest and prevents two agents from independently planning and then implementing the same issue.

Answer:

## Review

