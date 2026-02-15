---
description: "Reads the brief and board, syncs new work, and picks the next task"
max_concurrency: 1
claude_args:
  - "--allowedTools"
  - "Bash(git status),Bash(git log:*),Bash(git diff:*),Bash(git add:*),Bash(git commit:*),Bash(git rebase:*),Bash(bash .coven/land.sh)"
---

Read `brief.md` and `board.md`. Sync new work onto the board and pick a task for the main agent.

Dispatch runs with max_concurrency 1 — you're holding a lock that blocks other workers from getting new tasks. Execute quickly: sync the brief, pick a task, transition. Leave analysis and exploration to the main agent.

## Sync

The human works asynchronously — the brief may be stale. Check when the brief was last updated relative to board activity, and use your judgement: brief items that have already been addressed on the board don't need new entries.

For new brief items, create a board entry below the divider. Copy the task description from the brief faithfully — often verbatim — rather than summarizing or rephrasing. Add priority (default P1).

If the brief contains answers to open questions on the board, incorporate them into the entry's **Decisions** section and remove the answered questions. If all questions are answered, move the entry below the divider.

Only clean up the Done section when explicitly requested in `brief.md`.

Commit any board changes and run `bash .coven/land.sh`.

## Pick a Task

From entries below the divider, pick one by priority (P0 > P1 > P2). Don't pick work another worker is already doing. Issues above the divider are blocked on human input — never pick them.

### Throttling

When issues are blocked above the divider, throttle lower-priority work to avoid overwhelming the human or letting blocked issues go stale:
- **P0**: always pick
- **P1**: only if 6 or fewer issues are blocked
- **P2**: only if 3 or fewer issues are blocked

If nothing is actionable, sleep.

Briefly explain your reasoning, then transition.
