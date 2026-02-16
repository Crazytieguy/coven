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

Your job is exploration and decision-making, not implementation. Read the issue, understand the codebase, and figure out what needs to happen.

Post a concise plan to the board entry:
- **Decisions** — design choices you've made (approach, architecture, key trade-offs)
- **Questions** — things only the human can answer (requirements, preferences, scope)

Keep it short. The human needs to see key decisions and open questions — not implementation details they don't care about. If the path forward is obvious, say so briefly and ask "good to proceed?"

Move the issue under `# Blocked`, commit, land, and transition to dispatch.

## Recording Issues

If you notice unrelated problems (bugs, tech debt, improvements), add a new entry to `board.md` under `# Plan` with an appropriate priority. Don't stop your current work to address them.

## Rules

- **No code modifications.** You explore and plan — implementation happens later.
- **Land before transitioning to dispatch.** The worktree must not be ahead of main when returning to dispatch.
- Delete `scratch.md` on every land.
