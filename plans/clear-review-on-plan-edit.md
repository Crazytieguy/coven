Issue: workflow issue: when claude edits a reviewed plan, it should remove the review
Status: draft

## Approach

Add a rule to `workflow.md` under the "Acting on reviewed plans" section (priority 3) that when revising a rejected plan, Claude must clear the Review section (reset it to the empty template) after making changes. This ensures the human can tell at a glance which plans need re-review.

### Change

In `workflow.md`, under the `Status: rejected` bullet, append:

> After revising, clear the Review section back to the empty template so the human knows it needs re-review.

### Current text (line 9):
```
   - `Status: rejected` — revise the plan based on the Review section comments. Counts as one action.
```

### Proposed text:
```
   - `Status: rejected` — revise the plan based on the Review section comments. After revising, clear the Review section so the human knows it needs re-review. Counts as one action.
```

## Questions

None — this is straightforward.

## Review

