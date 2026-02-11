---
priority: P1
state: approved
---

# Dispatch agent doesn't mention when to invoke the audit agent

The dispatch agent prompt has routing rules for `plan` and `implement` agents based on issue state, but never mentions when the `audit` agent should be invoked.

The audit agent should be dispatched when:
- There are no issues with state `new` or `approved` (i.e., nothing to plan or implement)
- There are no more than 10 issues with state `review` (don't audit if the review queue is already full)

This gives workers useful work to do during idle periods instead of just sleeping.
