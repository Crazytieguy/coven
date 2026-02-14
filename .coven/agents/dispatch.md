---
description: "Reads the brief and board, syncs new work, and picks the next task"
max_concurrency: 1
claude_args:
  - "--allowedTools"
  - "Bash(git status),Bash(git log:*),Bash(git diff:*),Bash(git add:*),Bash(git commit:*),Bash(git rebase:*),Bash(bash .coven/land.sh)"
---

Read `brief.md` and `board.md`. Sync new work onto the board and pick a task for the main agent.

## board.md Format

```markdown
## P1: Issue title

Short description.

**Decisions:**
- Resolved question or design choice

**Questions:**
- Something needing human input

---

## P2: Another issue

Ready to implement.

## Done

- P1: Completed issue title
- P2: Another completed issue
```

- H2 per issue with priority in title
- Issues **above** the `---` divider need human input (open questions)
- Issues **below** the divider are ready or in progress
- Completed issues move to the `## Done` section as a single-line list item
- Only clean up the Done section when explicitly requested in `brief.md`

## Sync

Compare the brief against the board (including the Done section). For each brief item that doesn't have a board entry and isn't in Done, create a new board entry below the divider. Copy the task description from the brief faithfully — often verbatim — rather than summarizing or rephrasing. Add priority (default P1).

If the brief explicitly requests cleaning up the Done section, remove the specified entries (or all entries) from it.

If the brief contains answers to open questions on the board, incorporate them into the entry's **Decisions** section and remove the answered questions. If all questions are answered, move the entry below the divider.

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
