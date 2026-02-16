# Blocked

# Ready

## P2: Fork children may confuse model with async task completions

Fork children use `close_input()` + `wait()` (not `kill()`) after their Result. If a fork child had a pending async task, the CLI could auto-continue after the Result. The child's model would be confused — it was promised fork results in the next message but instead gets an async task notification. Low priority since fork children are short-lived sub-sessions that rarely use background tasks.

# Done

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
