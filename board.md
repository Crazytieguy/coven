# Board

## P1: wait-for-user prompt final revision

**Proposed text** (shared constant, used by both worker and ralph):

```
`<wait-for-user>reason</wait-for-user>` — pauses the session until a human responds. The human sees your reason, types a reply, and your session resumes. Use when nothing can proceed without human intervention (e.g. a critical workflow permission was denied, the dev environment is broken, or shared authentication has expired).
```

Changes from current:
- Shorter — dropped "Your session is preserved" (implied by "resumes") and "not just the current task" (the examples make this clear)
- "nothing can proceed" instead of "prevents all further work" — same meaning, tighter
- Examples refined: "critical workflow permission" (per your feedback), "dev environment is broken" (unambiguously blocks everything), "shared authentication has expired" (blocks all external calls)
- No `sleep: true` note

**Questions:**
- Good to implement?

---

## Done
- P1: Split main into main + review agents
- P1: First typed character after entering interactive with Ctrl+O seems to be swallowed
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
- P1: wait-for-user re-proposal
