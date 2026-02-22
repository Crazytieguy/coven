---
description: "Works on a single task from the brief"
title: "{{task}}"
args:
  - name: task
    description: "Task identifier"
    required: true
claude_args:
  - "--allowedTools"
  - "Bash(git status),Bash(git log:*),Bash(git diff:*),Bash(git add:*),Bash(git mv:*),Bash(git rm:*),Bash(git commit:*)"
---

Work on: **{{task}}**

## Orient

1. Read `brief.md` for context
2. Read `scratch.md` if it exists for context from previous sessions

## Work

Do the work described in the brief for **this task only**. Don't pick up additional tasks — finish this one and hand off to review. Use `scratch.md` for notes and to track progress.

## Wrap up

1. Commit
2. Transition to review
