---
description: "Reads the brief and board, syncs new work, and picks the next task"
max_concurrency: 1
claude_args:
  - "--allowedTools"
  - "Bash(git status),Bash(git log:*),Bash(git diff:*),Bash(git add:*),Bash(git commit:*),Bash(git rebase:*),Bash(bash .coven/land.sh)"
---

Read `brief.md` and `board.md`. Sync new work onto the board and pick a task for the main agent.

## Sync

Compare the brief against the board and `git log --oneline -20`. For each brief item that doesn't have a board entry and wasn't recently completed, create a new H2 entry on board.md below the divider with the task description and priority (default P1).

If the brief contains answers to open questions on the board, incorporate them into the entry's **Decisions** section and remove the answered questions. If all questions are answered, move the entry below the divider.

Clean up stale content from board entries: remove old design explorations, resolved alternatives, lengthy pro/con lists, and code examples that are no longer needed. An entry below the divider should have a short description plus its **Decisions** section — nothing more.

Commit any board changes and run `bash .coven/land.sh`.

## Pick a Task

From entries below the divider, pick one by priority (P0 > P1 > P2). Don't pick work another worker is already doing.

### Throttling

If there are open questions above the divider, throttle lower-priority work:
- **P0**: always pick
- **P1**: only if 6 or fewer issues are waiting for answers
- **P2**: only if 3 or fewer issues are waiting for answers

If nothing is actionable, sleep.

Briefly explain your reasoning, then transition.
