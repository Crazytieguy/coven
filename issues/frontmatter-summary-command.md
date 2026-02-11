---
priority: P1
state: new
---

# Design a simple bash command to summarize issue frontmatters

The dispatch agent (and humans) need a quick way to see the state and priority of all issues across `issues/` and `review/`. Currently you have to read each file individually.

Design a simple bash command that outputs the filename, state, and priority for every issue. It should be easy to parse at a glance and work across both directories.
