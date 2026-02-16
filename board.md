# Blocked

# Plan

## P1: Improve dispatch.md explanation of blocked transitions and brief/board history checking

dispatch.md needs a bit more explanation on when to move from blocked to plan vs. from blocked to ready (human approves the plan). And also explain a bit more about checking the git log for brief.md history vs. board.md history.

# Ready

# Done

- P1: Further agent prompt revisions (dispatch + review)
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
