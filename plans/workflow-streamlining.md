Issue: The priority based workflow causes a lot of duplicate work in the case that all the tasks are planned. Should probably streamline and make everything a task. Document planned feature to have recurring tasks
Status: draft

## Approach

Restructure `workflow.md` to reduce wasted work when all issues are already planned. Currently:

1. Lint (quick, fine)
2. Plan an unplanned issue (skipped when all are planned)
3. Act on reviewed plans (fallback reads ALL plan files every session — expensive and repetitive)
4-6. Review/test/refactor (rarely reached because 3 consumes the session)

### Changes to workflow.md

**Simplify priority 3 (act on reviewed plans):**
- Keep git-status-based discovery as primary (fast, precise)
- For the fallback, instead of reading ALL plan files, only read plans that were not created by the current agent in the current loop (i.e., plans older than the loop start). This reduces redundant reads.
- Alternatively, simplify further: only act on plans discovered via git status (uncommitted modifications). Remove the "read all plans" fallback entirely — if the human hasn't reviewed anything, skip to priority 4.

**Add recurring tasks concept:**
- Add a new section `## Recurring Tasks` to workflow.md that lists tasks that should be checked periodically (e.g., "review test cases", "check for refactoring opportunities")
- These are distinct from issues — they don't get "resolved", they're ongoing maintenance activities
- Move priorities 4-6 (review test cases, add test coverage, refactor) into recurring tasks since they're already ongoing by nature

**Result:** The priority list becomes:
1. Lint
2. Plan an unplanned issue
3. Act on reviewed plans (git status only, no fallback)
4. Recurring tasks (rotate through them)

## Questions

### Should we remove the "read all plans" fallback entirely?

The fallback exists so that approved plans don't get missed if the agent doesn't notice them via git status. But in practice, this causes every session to re-read 10+ plan files when nothing has been reviewed.

Options:
- **Remove fallback entirely**: Rely solely on git status. Risk: if a plan is approved and committed, it won't be discovered. Mitigation: the human can leave the plan file uncommitted after review.
- **Keep fallback but throttle it**: Only do the fallback scan every N sessions (hard to track across sessions).
- **Keep fallback but limit scope**: Only read plans not created in the last 24 hours or similar heuristic.

Answer:

### Should recurring tasks rotate or be prioritized?

When we reach the "recurring tasks" priority level, should the agent:
- Pick whichever seems most valuable (current behavior, but subjective)
- Rotate through them in order (deterministic, ensures coverage)
- Track which was done most recently and pick the least-recent (best coverage but needs state)

Answer:
