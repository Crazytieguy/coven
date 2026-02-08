Issue: workflow improvement: after finishing a task, we should have claude spin up a review subagent and iterate until the review returns pristine
Status: draft

## Approach

This is a workflow.md change, not a code change. After implementing an approved plan (priority 3), the workflow should instruct Claude to spawn a review subagent (via the Task tool) before committing. The subagent reviews the diff for correctness, style, and regressions. If it finds issues, Claude fixes them and re-reviews. Only commit once the review is clean.

### Changes to workflow.md

In the "Act on reviewed plans" section (currently line 8), after "implement the plan", add a review step:

> After implementation, spawn a review subagent (Task tool, Explore or general-purpose type) with the git diff and relevant context. The subagent should check for: correctness, style consistency, missed edge cases, unnecessary changes, and test coverage. If the review surfaces issues, fix them and re-review. Only proceed to commit once the review returns clean.

The review subagent prompt template (for the workflow instructions) would be something like:

```
Review the following changes for a coven contribution. Check for:
- Correctness: does the code do what the issue asks?
- Style: consistent with project conventions?
- Unnecessary changes: anything beyond what was requested?
- Edge cases: obvious gaps?
- Test coverage: are changes tested?

Reply with either "LGTM" if everything looks good, or list specific issues to fix.
```

### What NOT to change

- No changes to coven's Rust code — this is purely workflow discipline
- No changes to the ralph system prompt — the review step is part of the workflow instructions that Claude reads from workflow.md at the start of each session
- The review only applies to priority 3 (implementing approved plans), not to planning, linting, or other priorities

## Questions

### Should the review also apply to refactoring (priority 6)?

Refactoring changes could also benefit from a review subagent. However, adding review to every action type might slow things down. The issue specifically says "after finishing a task" which most naturally maps to plan implementation.

Options:
1. Only for approved plan implementation (priority 3)
2. For any code-changing action (priorities 3, 6)
3. For all priorities that produce commits

Answer:

### Should the review subagent see the full diff or targeted files?

Options:
1. Pass `git diff` output — simple, complete, but potentially large
2. Pass `git diff --stat` plus targeted file reads — more focused but may miss context
3. Let the subagent explore freely with access to the repo — most thorough but slower

Answer:

### How many review iterations before giving up?

If the review keeps finding issues, we need a cap to prevent infinite loops.

Options:
1. Cap at 2 review rounds (review, fix, re-review) — if still not clean, revert and stop
2. Cap at 3 rounds — more generous but risks wasting iterations
3. No cap — trust the process, but risk burning through ralph iterations

Answer:

## Review

