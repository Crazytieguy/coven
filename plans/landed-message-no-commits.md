Issue: [P1] "Landed" message prints even when agent committed nothing. Agent should always commit something unless it's a dispatch agent. If a non-dispatch agent produces no commits, consider resuming the session to ask it to commit.
Status: draft

## Approach

### 1. Detect "no unique commits" before landing

Add a `has_unique_commits(worktree_path) -> Result<bool>` function to `worktree.rs` that checks whether the worktree branch has any commits ahead of main:

```
git rev-list --count <main_branch>..HEAD
```

If count is 0, there are no unique commits.

### 2. Skip landing when there's nothing to land

In `worker_loop` (worker.rs), between Phase 2 (agent) and Phase 3 (land), check `has_unique_commits`. If false:

- Print a message like `"Agent produced no commits — skipping land.\r\n"`
- Skip the `land_or_resolve` call entirely
- Continue to the next dispatch iteration

This is the minimal fix for the "Landed" message printing incorrectly.

### 3. Resume session to ask agent to commit (optional enhancement)

The issue mentions "consider resuming the session to ask it to commit." This could be implemented as:

- When no commits are detected, resume the agent session with a prompt like: `"You finished without committing any changes. Please commit your work before ending."`
- After the resumed session completes, re-check for unique commits
- If still no commits, skip land and move on

This adds complexity and a potential retry loop. Worth discussing whether this is desirable for v1.

## Questions

### Should we implement the "resume to ask for commit" behavior?

The issue mentions this as a possibility. Options:

1. **Just skip land** — simplest fix, prevents the misleading "Landed" message. Agent work is lost but the next dispatch cycle picks up the same issue.
2. **Resume once** — resume the session with a "please commit" prompt, retry land if commits appear. One retry, then give up.
3. **Resume with retry loop** — keep resuming until commits appear or a max retry count is hit.

Option 1 is the cleanest for now; options 2/3 can be added later as a separate issue if needed.

Answer:

## Review

