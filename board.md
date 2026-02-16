# Blocked

## P1: Revise agent prompts based on restructuring feedback

Proposed wording changes for each file below. VCR re-recording waits until end of implementation.

### system.md — Add "Recording Issues" after `## Rules`

Remove the identically-worded section from plan.md, implement.md, and review.md. Add once in system.md:

```markdown
## Recording Issues

If you notice unrelated problems (bugs, tech debt, improvements) while working, add a new entry to `board.md` under `# Plan` with an appropriate priority. Don't stop your current work to address them.
```

### dispatch.md — Preserve human reasoning in Sync

Add one sentence to the brief-answers paragraph. Change:

> If the brief contains answers to open questions on a blocked issue, incorporate them into the entry's **Decisions** section and remove the answered questions. Use your judgement…

To:

> If the brief contains answers to open questions on a blocked issue, incorporate them into the entry's **Decisions** section and remove the answered questions. **Preserve the human's reasoning and intent faithfully — capture not just what they decided, but why.** Use your judgement…

### plan.md — Reframe agent role; remove Recording Issues

Replace the `## Plan` section entirely. Current version says "exploration and decision-making" and "design choices you've made." New version:

```markdown
## Plan

Your job is exploration, not implementation. Read the issue, understand the codebase, and surface what the human needs to know before implementation begins.

Post a concise plan to the board entry:
- **Decisions** — your proposed approach, stated as choices the human can approve or push back on. Document important decisions even when the path seems obvious — this sets expectations for implementation.
- **Questions** — things only the human can answer (requirements, preferences, ambiguous scope)

Keep it short. The human needs to see key decisions and open questions — not implementation details they don't care about. If the path forward is straightforward, say so briefly and ask "good to proceed?"
```

Delete the `## Recording Issues` section (now in system.md).

### implement.md — Simplify to three sections

Merge "Continuation" into "Implement", remove "Recording Issues". Full proposed body (frontmatter unchanged):

```markdown
Implement the board issue: **{{task}}**

## Orient

1. Read `board.md` to find your issue entry under `# Ready`
2. Read `scratch.md` if it exists for context from previous sessions
3. Read relevant code to understand the problem

The plan has been approved — follow the decisions in the board entry.

## Implement

Do one focused piece of work, commit, and update `scratch.md` with what you did and what's next.

If more work remains, transition to implement again to continue. When done, transition to review.

## Rules

- **Land before transitioning to dispatch.** The worktree must not be ahead of main when returning to dispatch.
- Delete `scratch.md` on every land.
```

### review.md — Reframe; replace "push back"; add incidental issues

**Rename section** from "Judge: Land or Push Back" to "Evaluate".

**Replace "push back"** — the action discards all code and sends the issue back to the human. Alternatives:

1. **Reject** — clearest about the outcome (work is discarded)
2. **Refer back** — emphasizes it goes to the human, not the implementer
3. **Escalate** — emphasizes human judgment is needed
4. **Return** — neutral, generic
5. **Bounce** — informal but accurately describes the flow
6. **Send back** — plainest description
7. **Discard** — focuses on what happens to the code

**Add incidental issues note** inline (distinct wording from system.md's generic version).

Remove `## Recording Issues` section.

Full proposed body (frontmatter unchanged):

```markdown
Review the implementation for board issue: **{{task}}**

## Gather Context

1. Read `board.md` to find the original issue entry and its acceptance criteria / decisions
2. Read `scratch.md` for the implementer's notes on what was done
3. Run `git diff <main-worktree-branch>...HEAD` to see the full diff
4. Read any files that need closer inspection

## Evaluate

Assess the implementation against the plan's decisions and acceptance criteria.

**[CHOSEN TERM]** (discard work and refer to the human) if:
- The implementer made design decisions that should have been posted to the board first — e.g. chose between multiple valid approaches, interpreted ambiguous requirements, or added scope beyond what was asked
- The implementation doesn't match the issue's decisions
- There are significant quality issues that need a different approach

To [CHOSEN TERM]: `git reset --hard <main-worktree-branch>` to discard the implementation, update the board entry with what went wrong, move it under `# Blocked`, commit, land, and transition to dispatch.

**Land** if the approach is sound:
- Fix any quality issues you notice — bugs, missing edge cases, style problems, test gaps
- Commit improvements separately from the implementer's work

While reviewing, watch for incidental issues in the surrounding code — bugs, inconsistencies, or improvements unrelated to this task. Record them on the board under `# Plan` so they don't get lost.

## Landing

When the implementation passes review:
1. Move the board entry to the `# Done` section (single line: `- P1: Issue title`) and commit
2. Run `bash .coven/land.sh` — if conflicts, resolve and run again
3. Delete `scratch.md`
4. Transition to dispatch
```

**Decisions:**
- Recording Issues moves to system.md; removed from all agents individually. Review gets an inline variant about incidental issues in surrounding code.
- dispatch.md gets one bolded sentence about preserving reasoning.
- plan.md reframes decisions as proposals for human approval, not unilateral choices. Still documents decisions even when obvious.
- implement.md drops from 4 sections to 3 (Continuation merged into Implement).
- review.md section renamed from "Judge: Land or Push Back" to "Evaluate". "Push back" description replaced with "discard work and refer to the human" pending term choice.
- VCR re-recording happens at the end of implementation, not blocking prompt changes.

**Questions:**
- Which term for the review rejection action? See the 7 alternatives above. (I'd lean toward **reject** for clarity or **refer back** for accuracy about where it goes.)

## P2: Strengthen dispatch prompt — new items must go to Plan

In VCR recordings (priority_dispatch), haiku sometimes puts new brief items directly in Ready, bypassing Plan. The dispatch prompt says "create a board entry under `# Plan`" but the model takes shortcuts for simple tasks. Consider making the instruction more emphatic or adding a rule.

**Decisions:**
- Add an explicit constraint rule to the Sync section of `dispatch.md`: new items always go to Plan, never directly to Ready. Even if a task looks trivial, the human reviews plans before implementation begins.
- Keep it to one bolded sentence + short rationale — minimal change to the prompt.

**Questions:**
- Good to proceed?

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

