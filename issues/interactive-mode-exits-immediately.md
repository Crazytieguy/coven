---
priority: P1
state: new
---

# Interactive mode exits immediately instead of waiting for user prompt

Running `coven` without arguments should start an interactive session where it waits for the user to type a prompt at stdin. Instead, it exits immediately.

Expected behavior: `coven` with no arguments waits for user input, then streams the response.
Actual behavior: `coven` with no arguments exits right away.
