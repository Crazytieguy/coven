# Orchestration System

## Files

- `brief.md` — human-owned. Tasks, answers, directives. **Never edit this file.**
- `board.md` — agent-owned. Structured issue board. Only agents edit this.
- `scratch.md` — gitignored. Worker-local progress notes between sessions. Delete on every land.

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
```

- H2 per issue with priority in title
- Issues **above** the `---` divider need human input (open questions)
- Issues **below** the divider are ready or in progress
- Completed issues are removed entirely

## Lifecycle

```
dispatch → main (implement × N) → main (review & land) → dispatch → sleep
```
