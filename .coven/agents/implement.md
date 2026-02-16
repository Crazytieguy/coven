---
description: "Implements a planned board issue"
title: "{{task}}"
args:
  - name: task
    description: "Board entry title"
    required: true
claude_args:
  - "--allowedTools"
  - "Bash(git status),Bash(git log:*),Bash(git diff:*),Bash(git add:*),Bash(git mv:*),Bash(git rm:*),Bash(git commit:*),Bash(git rebase:*),Bash(bash .coven/land.sh)"
---

Implement the board issue: **{{task}}**

## Orient

1. Read `board.md` to find your issue entry under `# Ready`
2. Read `scratch.md` if it exists for context from previous sessions
3. Read relevant code to understand the problem

The plan has been approved — follow the decisions in the board entry.

## Implement

Do one focused piece of work, commit, and update `scratch.md` with what you did and what's next.

If more work remains, transition to implement again to continue. When done, transition to review.
