---
priority: P1
state: new
---

# Let coven agents define `claude` arguments

Agents should be able to declare extra arguments to pass to the `claude` CLI. This could be a frontmatter field in the agent template (e.g. `claude_args`).

Use this in the default agent templates to grant permissions each agent needs — for example, commonly used git operations and the `land.sh` script.
