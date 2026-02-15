# Board

## P1: Review break and wait-for-user state

### Current state

**Ralph** — both tags documented in the system prompt and fully functional:
- `<break>` ends the loop permanently (customizable via `--break-tag`, disableable via `--no-break`)
- `<wait-for-user>` pauses the loop, preserves the session, resumes with user input
- Priority: wait-for-user checked before break tag

**Worker** — `<wait-for-user>` is fully implemented in code but invisible to agents:
- `run_phase_with_wait()` handles the wait/resume loop correctly
- `parse_transition()` checks for `<wait-for-user>` before `<next>` (correct priority)
- But `format_transition_system_prompt()` only teaches agents `<next>` and `sleep: true`
- A test explicitly asserts the prompt does NOT mention wait-for-user
- The corrective prompt (for failed transition parsing) also doesn't mention it
- `.coven/system.md` and agent prompts (`main.md`, `dispatch.md`) don't mention it either

**Worker** — `<break>` is not referenced at all (not in prompts or code). This is expected — worker uses `<next>sleep: true</next>` instead.

### Issues

1. **Worker agents can't discover `<wait-for-user>`** — the mechanism works in code but agents are never told about it. If an agent happens to output this tag (e.g. from CLAUDE.md context leaking ralph conventions), it would work. But no agent will produce it intentionally.

2. **This is probably a regression** — per the board note, removing it from the worker prompt may have been a mistake. There are real use cases: permission denials, needing clarification, wanting to show progress before continuing. Currently, if a worker agent gets stuck, it has no way to pause and ask the human — it either succeeds, retries the transition, or crashes.

3. **Design question**: should `<wait-for-user>` be added to the transition system prompt alongside `<next>` and `sleep`? Or should it be documented as an "escape hatch" in `.coven/system.md` / agent prompts only? Adding it to the transition prompt risks agents over-using it (asking for confirmation when they should just proceed).

**Questions:**
- Want me to add `<wait-for-user>` back to the transition system prompt? If so, should it be presented as a peer of `<next>`/`sleep`, or as a last-resort escape hatch with strong guidance to prefer autonomous action?

---

## P1: self_transition_review test doesn't trigger a review session

The main agent completes trivial tasks in a single session without self-transitioning for review. The test may need a harder task, or the prompt may need to better encourage review sessions.

## Done
