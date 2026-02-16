# Blocked

# Ready

## P1: Agent restructuring — split main into plan + implement

Split the main agent into two: plan and implement. Issues default to needing planning. New lifecycle:
- Issue added via brief, marked as needs planning (via dispatch)
- Planning → results in questions/decisions for the human (via plan agent)
- Human answers questions, dispatch infers whether issue still needs planning or can transition to implementation ready
- Either re-plan or implement

Plans should be concise: key decisions and open questions only, no irrelevant implementation details. Implementation agent keeps an escape hatch to add questions if needed. Agent-added issues default to needing planning. All agents should be empowered to add issues liberally.

Human wants to be involved in prompting decisions — propose specifics for review.

## P2: Parent session may auto-continue during fork execution

When the parent outputs `<fork>`, coven runs fork children then sends the reintegration message back. While fork children are running, the parent CLI is idle — if an async task completes, the parent auto-continues and may change state before receiving the expected fork results.

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
- P1: Mark a session to wait for user input when it finishes
