---
priority: P2
state: new
---

# Investigate race condition: commits on main missed during sleep setup

When a worker decides to sleep and starts watching main refs for new commits, there may be a window where a commit lands after the agent started running (so it sees stale state) but before the watcher is active — causing the worker to sleep indefinitely and miss the commit.

Investigate whether this race exists, and if so propose ways to handle it.
