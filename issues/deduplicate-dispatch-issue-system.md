---
priority: P1
state: approved
---

# Remove redundant issue system explanation from dispatch agent

The dispatch agent (`.coven/agents/dispatch.md`) re-explains the issue system (states, routing, priorities) even though `.coven/workflow.md` is automatically loaded into context. Remove the duplicated content and rely on the workflow doc.
