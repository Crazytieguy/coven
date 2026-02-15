# Board

---

## P1: Add wait-for-user to worker and ralph system prompts

Add `<wait-for-user>` to the built-in coven worker system prompt (not `.coven/system.md` — that's a template). Present it as a last resort that completely blocks the worker until a human is available. Same treatment for ralph. Next step: quick overview of the current prompting for this and some options.

**Decisions:**
- Add to built-in system prompt, not `.coven/system.md`
- Present as last resort (blocks worker until human available)
- Same approach for ralph
- `<break>` tag name is fine as-is

## P1: self_transition_review test doesn't trigger a review session

The main agent completes trivial tasks in a single session without self-transitioning for review. Need a slightly harder task — a slightly tricky algorithm that's still fast for VCR (single file generation). If it still doesn't trigger a self-transition to review, update the human and decide whether to go harder or change the prompt.

**Decisions:**
- Try a slightly harder task (not prompt changes). Keep it cheap for VCR — single file, slightly tricky algorithm.
- If it still fails, report back rather than iterating autonomously.

## Done
