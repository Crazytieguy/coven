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

If you hit ambiguity or a decision point that wasn't covered by the plan, stop — discard your un-landed code changes, update the board entry with your questions, move it under `# Blocked`, commit, land, and transition to dispatch.

## Continuation

If more work remains, transition to implement again to continue.

When implementation is complete, transition to review to get the work landed.

## Recording Issues

If you notice unrelated problems (bugs, tech debt, improvements), add a new entry to `board.md` under `# Plan` with an appropriate priority. Don't stop your current work to address them.

## Rules

- **Land before transitioning to dispatch.** The worktree must not be ahead of main when returning to dispatch.
- Delete `scratch.md` on every land.
