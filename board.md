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

Comments from the brief on each agent prompt:

**dispatch.md:**
- Can clarify a bit more that preserving human input and decisions faithfully is important (so implementation doesn't diverge from the plan)

**plan.md:**
- "## Plan" section: not accurate. It's more about understanding the requirements and finding ambiguity or inconsistency. The main goal for the plan agent is to elicit the human's preferences for implementation via bringing up the right questions
- "Recording Issues" section: since this applies to all agents, it should be in system.md instead (though the review agent should have emphasis)

**implement.md:**
- The "If you hit ambiguity" paragraph has been dropped: rely on review instead
- Can overall be simplified - too many small sections. What's important is that it transitions to review when it's done

**review.md:**
- Rather than framing as gating the implementer's work, frame as evaluating the changes. It's not about the implementer doing a good/poor job, it's about whether the changes match the criteria
- Maybe "push back" has a somewhat negative valence?
- Should have emphasis (with different wording from system.md) on noticing issues and adding them to the board: this is part of the review task (for issues that shouldn't block landing or are unrelated)

General note: VCR tests currently fail and will need re-recording after these changes. That's ok.

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

