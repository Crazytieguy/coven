---
priority: P2
state: new
---

# Don't re-print session ID when resuming a session

When resuming an existing session, the session ID is printed again redundantly. Since the user already saw it when the session started, there's no need to print it a second time on resume.
