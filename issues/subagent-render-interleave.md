---
priority: P2
state: new
---

# Subagent tool call rendering interleaves across lines

When multiple subagents are active, tool call lines sometimes get merged onto the same line instead of each printing on its own line:

```
[17] ▶ Task  Explore VCR test structure
[18] ▶ Task    [17/1] ▶ Bash  find /Users/yoav/.coven/worktrees/coven/cool-stream-28/tests -type f -name "*.rs" -o -type f -name "*.md" -o -type d | hea...
```

Expected: each line should be printed in full on its own line. The `[17/1] ▶ Bash ...` line should start on a new line, not appended to `[18] ▶ Task`.
