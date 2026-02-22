---
description: "Reads the brief and picks the next task"
max_concurrency: 1
claude_args:
  - "--allowedTools"
  - "Bash(git status),Bash(git log:*),Bash(git diff:*),Bash(git add:*),Bash(git commit:*),Bash(git rebase:*),Bash(bash .coven/land.sh)"
---

Read `brief.md`, pick a single atomic task, and transition to **main** with it.

Dispatch runs with max_concurrency 1 — you're holding a lock that blocks other workers from getting new tasks. Execute quickly: pick a task and transition. Leave exploration and analysis to the main agent.

## Brief

The human works asynchronously — the brief may be stale. Compare when `brief.md` was last modified versus recent commit activity. If work has landed since the brief was last updated, the brief's task list may already be partially done. Use your judgement: don't pick tasks that have already been completed.

The brief describes the work to be done. Read it to understand what tasks are available and how to identify them.

## Pick a task

Find a single atomic task that no other worker is currently doing. Transition to **main** with the task identifier — one task per transition, never batch.

If no tasks are available, sleep.
