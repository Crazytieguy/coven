Issue: Workflow: we should have issue priorities
Status: draft

## Approach

Add priority markers to issues in `issues.md` so that the autonomous workflow can pick the most important unplanned/approved issues first, rather than processing them in arbitrary order.

### Format change in issues.md

Each issue gets an optional priority prefix: `[P0]`, `[P1]`, or `[P2]`. Issues without a prefix default to `[P1]` (medium). Example:

```
- [P0] Critical bug: token count is wrong (plan: plans/token-overcounting.md)
- [P1] Duplicated error rendering (plan: plans/duplicated-error-rendering.md)
- Workflow: we should have issue priorities
```

### Changes to workflow.md

Update priorities 2 and 3 in `workflow.md` to account for issue priority:

- **Priority 2 (Plan an unplanned issue)**: "Pick the highest-priority unplanned issue (lowest P-number). If tied, pick the first one listed."
- **Priority 3 (Act on reviewed plans)**: "When multiple approved plans exist, act on the highest-priority one first."

### Tagging existing issues

As part of implementation, tag all current issues in `issues.md` with a priority. Suggested defaults:

- `[P1]` for most issues (the current batch of code quality / refactoring issues)
- `[P2]` for workflow meta-issues (the three unplanned ones)

The human can adjust these during review.

## Questions

### Priority scale

I'm proposing a simple three-level scale (P0/P1/P2). Is this sufficient, or would you prefer more levels? Three levels keep cognitive overhead low while still allowing meaningful ordering.

Answer:

### Default priority

Should untagged issues default to P1 (medium) or P2 (low)? Defaulting to P1 means the workflow naturally treats untagged issues as "normal" priority, while P2 would force explicit tagging to get normal handling.

Answer:

## Review

