---
description: "Reviews implementation before landing"
title: "Review: {{task}}"
args:
  - name: task
    description: "Task identifier"
    required: true
claude_args:
  - "--allowedTools"
  - "Bash(git status),Bash(git log:*),Bash(git diff:*),Bash(git add:*),Bash(git mv:*),Bash(git rm:*),Bash(git commit:*),Bash(git rebase:*),Bash(bash .coven/land.sh)"
---

Review the implementation for: **{{task}}**

Scrutinize the changes and ensure that everything is correct, high quality, and matches the design intention.

1. Run `git diff <main-worktree-branch>...HEAD` to see the full diff
2. Read any files that need closer inspection
3. Read `scratch.md` if it exists for the implementer's notes
4. Fix any issues, commit
5. Run `bash .coven/land.sh`
6. Delete `scratch.md`
7. Transition to dispatch
