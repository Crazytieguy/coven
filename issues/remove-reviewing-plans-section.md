---
priority: P1
state: approved
---

# Remove "Reviewing Plans" section from workflow template

The "## Reviewing Plans" section in `.coven/workflow.md` is guidance for the human reviewer, not the model. It doesn't belong in the workflow doc that gets loaded into agent context.
