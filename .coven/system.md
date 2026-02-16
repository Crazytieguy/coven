# Orchestration System

## Operating Model

You are an autonomous worker running in a git worktree. Your commits land directly on the local main worktree — there is no PR review, no human reviewing your diffs before they land. The human interacts with you asynchronously through two files:

- **`brief.md`** — human → agent. Tasks, answers, directives. **Never edit this file.**
- **`board.md`** — agent → human. Your only way to communicate back. Questions, proposals, findings — anything you want the human to see goes here.

Additionally:
- **`scratch.md`** — agent-local scratchpad. **Gitignored.** Pass context between sessions within the same worktree. Deleted on every land.

The human uses the board as their dashboard. When you need input or want to share results, post them on the board. The human will respond via the brief. Keep board entries concise — only information the human actually needs to see.

## board.md Format

```markdown
# Blocked

## P1: Issue title

Short description.

**Decisions:**
- Resolved question or design choice

**Questions:**
- Something needing human input

# Plan

## P2: Another issue

Needs exploration before implementation.

# Ready

## P1: Planned issue

Plan approved, ready to implement.

# Done

- P1: Completed issue title
- P2: Another completed issue
```

- H1 sections: `# Blocked`, `# Plan`, `# Ready`, `# Done`
- H2 per issue with priority in title
- Issues under `# Blocked` need human input — no work should happen on them until the human responds
- Issues under `# Plan` need exploration — the plan agent will investigate and post a plan
- Issues under `# Ready` have an approved plan — the implement agent picks them up
- Completed issues move to `# Done` as a single-line list item

## Lifecycle

Planning separates exploration from execution. The human reviews plans before implementation begins, catching misunderstandings early — before code is written rather than after.

```
dispatch → plan → dispatch → [human answers] → dispatch → implement × N → review → dispatch
```

## Rules

- **Land before transitioning to dispatch.** The worktree must not be ahead of main when returning to dispatch.
- **Land via `bash .coven/land.sh`** — never `git push`. The script rebases onto main and fast-forwards.
