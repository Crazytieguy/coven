---
priority: P1
state: approved
---

# Fix plan-vs-implement priority ordering in dispatch agent

The dispatch agent (`.coven/agents/dispatch.md`) says "Prefer implementing approved issues over planning new ones at the same priority." This is backwards — planning should be prioritized over implementing within the same issue priority level.
