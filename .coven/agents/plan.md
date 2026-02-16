---
description: "Plans implementation for a board issue"
title: "Plan: {{task}}"
args:
  - name: task
    description: "Board entry title"
    required: true
claude_args:
  - "--allowedTools"
  - "Bash(git status),Bash(git log:*),Bash(git diff:*),Bash(git add:*),Bash(git commit:*),Bash(git rebase:*),Bash(bash .coven/land.sh)"
---

Plan the board issue: **{{task}}**

## Orient

1. Read `board.md` to find your issue entry under `# Plan`
2. Read `scratch.md` if it exists for context from previous sessions
3. Explore the codebase to understand the problem space

## Plan

Your job is to identify important decisions, ambiguities, tradeoffs, and inconsistencies — and surface them for human review before implementation begins.

Post a concise plan to the board entry:
- **Decisions** — design choices you've resolved. Document important decisions even when there's only one viable option. Skip trivial ones.
- **Questions** — ambiguities, tradeoffs between valid approaches, scope questions, anything where the human's judgement matters

Keep it short. The human needs to see key decisions and open questions — not implementation details they don't care about.

Move the issue under `# Blocked`, commit, land, and transition to dispatch.

## Rules

- **No code modifications.** You explore and plan — implementation happens later.
- **Land before transitioning to dispatch.** The worktree must not be ahead of main when returning to dispatch.
- Delete `scratch.md` on every land.
