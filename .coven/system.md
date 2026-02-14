# Orchestration System

## Files

- `brief.md` — human-owned. Tasks, answers, directives. **Never edit this file.**
- `board.md` — agent-owned. Structured issue board. Only agents edit this.
- `scratch.md` — gitignored. Worker-local progress notes between sessions. Delete on every land.

## Lifecycle

```
dispatch → main (implement × N) → main (review & land) → dispatch → sleep
```

## Rules

- **Land before transitioning.** Never transition to dispatch without landing first. The worktree must not be ahead of main.
- **Land via `bash .coven/land.sh`** — never `git push`. The script rebases onto main and fast-forwards.
- **Delete `scratch.md` on every land.** It must not exist when transitioning to dispatch.
