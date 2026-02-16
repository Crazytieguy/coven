# Blocked

## P1: Further agent prompt revisions (dispatch + review)

History audit (14 commits, 84c8993→5bbdd89): both requested changes were already implemented in the previous P1. The brief was written after the implementation landed, quoting text that had already been changed. No decisions were lost — earlier proposals (e.g. "preserve human's reasoning" sentence, review incidental issues note) were refined away during planning rounds, not dropped accidentally.

Current state:
- **dispatch.md**: "Use your judgement..." → already replaced with "Move the issue to `# Plan` unless the human explicitly says to proceed."
- **review.md**: "Improve and land" section already has the exact shorter text from the brief.

**Decisions:**
- "Strengthen dispatch prompt" issue dropped — subsumed by this

**Questions:**
- Both specific changes you asked for are already in place. Should this move to Done, or are there additional simplifications you want? (If so, what specifically — the current dispatch prompt is 44 lines, the review is 46.)

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
