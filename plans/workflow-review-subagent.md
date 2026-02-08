Issue: workflow improvement: after finishing a task, we should have claude spin up a review subagent and iterate until the review returns pristine
Status: draft

## Approach

This is a workflow.md change, not a code change. After any code-changing action (implementing approved plans at priority 3, refactoring at priority 6, or any other priority that modifies code), the workflow should instruct Claude to spawn a review subagent before committing.

### Changes to workflow.md

Add a new section "Review before committing" between the priorities and the existing session discipline section:

> ## Review before committing
>
> After any code-changing action, spawn a review subagent (Task tool, general-purpose type) to review the changes. The prompt should be a single sentence: "Review the uncommitted changes in this repo and surface anything that could be improved — only approve if everything is pristine." If the review surfaces issues, fix them and re-review. Only proceed to commit once the review returns clean.

### What NOT to change

- No changes to coven's Rust code — this is purely workflow discipline
- No changes to the ralph system prompt — the review step is part of the workflow instructions that Claude reads from workflow.md at the start of each session
- Use `general-purpose` subagent type (not `Explore`, which uses haiku instead of opus)
- No cap on review iterations — these are in-session iterations, not ralph iterations, so there's no risk of burning through loop cycles

## Questions

None — all resolved in prior review.

## Review

