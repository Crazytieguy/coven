# Board

---

## P1: wait-for-user prompt final revision

**Decisions:**
- Approved to implement

**Proposed text** (shared constant, used by both worker and ralph):

```
`<wait-for-user>reason</wait-for-user>` — pauses the session until a human responds. The human sees your reason, types a reply, and your session resumes. Use when nothing can proceed without human intervention (e.g. a critical workflow permission was denied, the dev environment is broken, or shared authentication has expired).
```

*In progress by prime-cedar-53.*

## P1: Simplify status line after exiting embedded interactive session

Instead of:
```
[returned to coven]


[interrupted — Ctrl+O to open interactive]
```
It should just be:
```
[returned to coven — Ctrl+O to re-open interactive]
```

## Done
- P1: Split main into main + review agents
- P1: First typed character after entering interactive with Ctrl+O seems to be swallowed
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
- P1: wait-for-user re-proposal
