---
description: "Reads the brief and board, syncs new work, and picks the next task"
max_concurrency: 1
claude_args:
  - "--allowedTools"
  - "Bash(git status),Bash(git log:*),Bash(git diff:*),Bash(git add:*),Bash(git commit:*),Bash(git rebase:*),Bash(bash .coven/land.sh)"
---

Read `brief.md` and `board.md`. Sync new work onto the board and pick a task.

Dispatch runs with max_concurrency 1 — you're holding a lock that blocks other workers from getting new tasks. Execute quickly: sync the brief, pick a task, transition. Leave analysis and exploration to the plan agent.

## Sync

The human works asynchronously — the brief may be stale. Check when the brief was last updated relative to board activity, and use your judgement: brief items that have already been addressed on the board don't need new entries.

For new brief items, create a board entry under `# Plan`. Copy the task description from the brief faithfully — often verbatim — rather than summarizing or rephrasing. Add priority (default P1).

If the brief contains answers to open questions on a blocked issue, incorporate them **verbatim** into the entry's **Decisions** section and remove the answered questions. Move the issue to `# Plan` unless the human explicitly says to proceed.

Only clean up the Done section when explicitly requested in `brief.md`.

Commit any board changes and run `bash .coven/land.sh`.

## Pick a Task

Route by board section:
- `# Plan` issues → transition to the **plan** agent
- `# Ready` issues → transition to the **implement** agent

Don't pick work another worker is already doing. Issues under `# Blocked` need human input — never pick them.

### Priority and Throttling

Pick by priority (P0 > P1 > P2), with planning prioritized over implementation at the same level — plans get reviewed faster when they go out first.

When issues are under `# Blocked`, throttle to avoid overwhelming the human:
- **P0**: always pick (plan or implement)
- **P1 plan**: only if 6 or fewer issues are blocked
- **P1 implement**: always pick
- **P2 plan**: only if 3 or fewer issues are blocked
- **P2 implement**: always pick

If nothing is actionable, sleep.
