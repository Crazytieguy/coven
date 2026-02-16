# Blocked

## P2: Strengthen dispatch prompt — new items must go to Plan

In VCR recordings (priority_dispatch), haiku sometimes puts new brief items directly in Ready, bypassing Plan. The dispatch prompt says "create a board entry under `# Plan`" but the model takes shortcuts for simple tasks. Consider making the instruction more emphatic or adding a rule.

**Decisions:**
- Add an explicit constraint rule to the Sync section of `dispatch.md`: new items always go to Plan, never directly to Ready. Even if a task looks trivial, the human reviews plans before implementation begins.
- Keep it to one bolded sentence + short rationale — minimal change to the prompt.

**Questions:**
- Good to proceed?

# Plan

## P1: Revise agent prompts based on restructuring feedback

Re-draft proposed wording for each agent prompt based on human feedback. VCR re-recording waits until end of implementation.

**Decisions:**
- Review term: **refer back**. Make the review about the changes, not the implementer.
- dispatch.md: simplify — just tell it to preserve **Decisions** and human input verbatim, no need for the elaborate "reasoning and intent" sentence. Re-draft.
- plan.md: "exploration, not implementation" is still not accurate. The goal is to identify important decisions, ambiguities, tradeoffs, inconsistencies, and surface them for review. Questions section can be expanded a bit. Remove the "if the path forward is straightforward" shortcut. Reframe "even when obvious" to "only one viable option" — document important decisions even when there's only one viable option, but don't document trivial ones.
- implement.md: it doesn't transition to dispatch and so it never needs to land. It also doesn't need to delete scratch.md. These are for the review agent. Simplify even further.
- review.md: "refer back" chosen. Remove "The implementer made" framing — make it about the changes. "Fix any quality issues" should be more about quality — mention dry, redundant or inconsistent comments, and generically "project guidelines" so it adapts to CLAUDE.md.
- Recording Issues still moves to system.md.
- VCR re-recording happens at the end of implementation, not blocking prompt changes.

# Ready

# Done

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

