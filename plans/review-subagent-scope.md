Issue: Workflow issue: the review subagent should not look at plans/ or issues.md
Status: draft

## Approach

Update the review subagent prompt in `workflow.md` to explicitly exclude workflow artifacts (`plans/`, `issues.md`) from review scope. The subagent should only review code, test, and config changes.

### Change

In `workflow.md`, line 46, change the review subagent prompt from:

```
"Review the uncommitted changes in this repo and surface anything that could be improved — only approve if everything is pristine."
```

To:

```
"Review the uncommitted changes in this repo and surface anything that could be improved — only approve if everything is pristine. Ignore changes to plans/, issues.md, and workflow.md — these are workflow artifacts, not code."
```

This is a single-line change in `workflow.md`.

## Questions

None — this is straightforward.

## Review

