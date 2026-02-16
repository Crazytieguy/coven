# Blocked

## P2: Strengthen dispatch prompt — new items must go to Plan

In VCR recordings (priority_dispatch), haiku sometimes puts new brief items directly in Ready, bypassing Plan. The dispatch prompt says "create a board entry under `# Plan`" but the model takes shortcuts for simple tasks. Consider making the instruction more emphatic or adding a rule.

**Decisions:**
- Add an explicit constraint rule to the Sync section of `dispatch.md`: new items always go to Plan, never directly to Ready. Even if a task looks trivial, the human reviews plans before implementation begins.
- Keep it to one bolded sentence + short rationale — minimal change to the prompt.

**Questions:**
- Good to proceed?

## P1: Revise agent prompts based on restructuring feedback

**Decisions:**
- **system.md:** Add a "Recording Issues" section (moved from individual agents). All agents get the instruction; review.md gets additional emphasis with its own wording.
- **dispatch.md:** Add a note about preserving the human's decisions and intent faithfully when incorporating brief answers into board entries — not just task descriptions, but the reasoning behind decisions.
- **plan.md:** Rewrite the "Plan" section. Current framing ("exploration and decision-making") positions the agent as the decision-maker. Reframe: the agent explores to find ambiguity, surfaces trade-offs, and asks the right questions to elicit the human's preferences. "Decisions" in the board entry are things the agent proposes for human approval, not unilateral choices.
- **implement.md:** Simplify. Merge "Continuation" into "Implement", remove "Recording Issues" (now in system.md). Three sections: Orient, Implement, Rules. Key message: follow the plan, commit incrementally, transition to review when done.
- **review.md:** Reframe from "judge/gate" to "evaluate the changes against the plan." Replace "push back" with "request changes." Add emphasis on noticing incidental issues and recording them on the board as part of the review task (distinct wording from system.md).
- **VCR re-recording** not part of this task — tests are expected to fail until a separate re-record pass.

**Questions:**
- For review.md, is "request changes" the right replacement for "push back"? Other options: "return for revision", "send back."
- For plan.md, should the agent still be able to state obvious decisions (e.g. "the obvious approach is X, good to proceed?") or should it always frame everything as questions?

# Plan

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

