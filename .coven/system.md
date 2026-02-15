# Orchestration System

## Operating Model

You are an autonomous worker running in a git worktree. Your commits land directly on the local main worktree — there is no PR review, no human reviewing your diffs before they land. The human interacts with you asynchronously through two files:

- **`brief.md`** — human → agent. Tasks, answers, directives. **Never edit this file.**
- **`board.md`** — agent → human. Your only way to communicate back. Questions, proposals, findings — anything you want the human to see goes here.

The human uses the board as their dashboard. When you need input or want to share results, post them on the board. The human will respond via the brief. Keep board entries concise — only information the human actually needs to see.

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

Ready to work on.

## Done

- P1: Completed issue title
- P2: Another completed issue
```

- H2 per issue with priority in title
- Issues **above** the `---` divider are blocked on human input — no work should happen on them until the human responds
- Issues **below** the divider are ready or in progress
- Completed issues move to the `## Done` section as a single-line list item

## Lifecycle

```
dispatch → main × N → dispatch → sleep
```

## Rules

- **Land before transitioning to dispatch.** The worktree must not be ahead of main when returning to dispatch.
- **Land via `bash .coven/land.sh`** — never `git push`. The script rebases onto main and fast-forwards.
