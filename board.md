# Blocked

# Plan

# Ready

## P1: Revise agent prompts based on restructuring feedback

Draft wording for each prompt revision. VCR re-recording waits until end of implementation.

### dispatch.md

One change in Sync — add "verbatim" to the brief-answers paragraph:

> If the brief contains answers to open questions on a blocked issue, incorporate them **verbatim** into the entry's **Decisions** section and remove the answered questions. Use your judgement…

Also simplify the "use your judgement on where to move the issue" paragraph — should mostly defer to the human rather than the agent deciding.

### plan.md

Replace the `## Plan` section:

```
## Plan

Your job is to identify important decisions, ambiguities, tradeoffs, and inconsistencies — and surface them for human review before implementation begins.

Post a concise plan to the board entry:
- **Decisions** — design choices you've resolved. Document important decisions even when there's only one viable option. Skip trivial ones.
- **Questions** — ambiguities, tradeoffs between valid approaches, scope questions, anything where the human's judgement matters

Keep it short. The human needs to see key decisions and open questions — not implementation details they don't care about.
```

Remove `## Recording Issues` (moving to system.md).

### implement.md

Remove Rules section entirely (it only transitions to itself or review — never lands, never deletes scratch.md). Full body after frontmatter:

```
Implement the board issue: **{{task}}**

## Orient

1. Read `board.md` to find your issue entry under `# Ready`
2. Read `scratch.md` if it exists for context from previous sessions
3. Read relevant code to understand the problem

The plan has been approved — follow the decisions in the board entry.

## Implement

Do one focused piece of work, commit, and update `scratch.md` with what you did and what's next.

If more work remains, transition to implement again to continue. When done, transition to review.
```

### review.md

Rename section to "Evaluate". Use "refer back." Make it about the changes. Shorter "improve and land":

```
## Evaluate

Assess the implementation against the plan's decisions.

**Refer back** (discard work and post to the board) if:
- The changes include design decisions that should have been posted to the board first — e.g. choosing between multiple valid approaches, interpreting ambiguous requirements, or adding scope beyond what was asked
- The implementation doesn't match the issue's decisions
- There are significant quality issues that need a different approach

To refer back: `git reset --hard <main-worktree-branch>` to discard the implementation, update the board entry with what went wrong, move it under `# Blocked`, commit, land, and transition to dispatch.

**Improve and land** if the approach is sound:
- Fix quality issues
- Simplify, dry
- Clean up redundant or inconsistent comments
- Check against project guidelines
- Commit
```

Remove `## Recording Issues`.

### system.md

Add after `## Rules`:

```
## Recording Issues

If you notice unrelated problems (bugs, tech debt, improvements) while working, add a new entry to `board.md` under `# Plan` with an appropriate priority. Don't stop your current work to address them.
```

**Decisions:**
- All prior decisions preserved (review term "refer back", plan reframing, implement simplification, Recording Issues to system.md).
- dispatch.md: simplify the "use your judgement" move-issue paragraph — defer to human rather than agent deciding.
- review.md: shorter "improve and land" list per human's wording.
- "Strengthen dispatch prompt" issue dropped — subsumed by these revisions.

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

