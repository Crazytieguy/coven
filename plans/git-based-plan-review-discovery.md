Issue: Since we're now committing plans and issues immediately, claude can find reviewed plan just using git (I don't commit plans when I review them)
Status: draft

## Approach

Update `workflow.md` priority 3 to instruct the agent to use `git status` (or the gitStatus provided at session start) to identify which plan files have been modified since the last commit. Since plans are committed immediately after creation, any uncommitted modifications to plan files indicate the human has reviewed them.

### Changes

1. In `workflow.md`, revise the priority 3 description from "Read each plan file referenced from issues.md" to something like: "Check git status for modified plan files — these are the ones the human has reviewed. Read only those files and act on their updated status."

This is a small wording change to workflow.md — no code changes needed.

### Benefit

- Avoids reading all plan files every iteration (minor efficiency gain)
- Makes the review discovery mechanism explicit and self-documenting
- Aligns the workflow description with how it actually works in practice

## Questions

### Should the agent also fall back to reading all plans?

Git-based discovery only works if the human reviews by editing the file locally without committing. If the human ever commits their review (e.g., by accident or to share with someone), the git signal would be lost. Should we keep the old approach as a fallback, or is it safe to rely entirely on git status?

Answer:

## Review

