# Blocked

# Blocked

## P1: Further agent prompt revisions (dispatch + review)

Both changes requested here were already implemented in the previous P1 ("Revise agent prompts based on restructuring feedback"):

1. **dispatch.md** — the old "Use your judgement on where to move the issue..." text is already replaced with: "Move the issue to `# Plan` unless the human explicitly says to proceed."
2. **review.md** — the "improve and land" section already has exactly the shorter text from the brief.

Audited full board.md + brief.md history (14 commits across the multi-round planning process). All decisions were implemented — nothing was lost.

This issue appears to have been created from stale brief content that was already consumed.

**Questions:**
- Should this move straight to Done, or are there additional dispatch/review changes you want?

# Plan

# Ready

# Done

- P1: Revise agent prompts based on restructuring feedback

- P1: Agent restructuring — split main into plan + implement

- P1: Investigate follow-up messages vs. next tag (findings below)
- P2: Fix parent auto-continue during fork (kill parent CLI before fork children run, respawn with reintegration message after)
- P1: Fix invisible claude sessions (kill CLI after Result in worker/ralph to prevent async task continuations)
- P1: Coordinate worker sleep — if one dispatch sleeps, others should too

- P1: Review: is `git reset --hard main` correct in the review agent?
- P1: Implement new board format (replace divider with Blocked/Ready sections)
- P2: Capture stderr from claude process
- P1: Split main into main + review agents
- P1: First typed character after entering interactive with Ctrl+O seems to be swallowed
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
- P1: wait-for-user re-proposal
- P1: Simplify status line after exiting embedded interactive session
- P1: wait-for-user prompt final revision
- P2: scratch.md: should clarify that it's gitignored
- P1: Mark a session to wait for user input when it finishes

