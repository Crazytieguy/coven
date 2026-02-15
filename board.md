# Board

---

## P1: self_transition_review test doesn't trigger a review session

Tried a harder task (merge_intervals — sorting, merging overlapping/adjacent intervals, edge cases). Updated the fixture and re-recorded. Haiku still completes everything in one main session — it inlines the review rather than self-transitioning to a fresh context.

**Decisions:**
- Improve the prompt rather than dropping the requirement
- Explain the "why": a review with a fresh context window catches issues that could be missed — like fresh eyes
- Prefer explaining the "why" over ALL CAPS instructions
- Make the task slightly harder as well (safety buffer)
- Propose several prompting options for human to choose from

## P1: Add wait-for-user to worker and ralph system prompts

Add `<wait-for-user>` to the built-in coven worker system prompt (not `.coven/system.md` — that's a template). Present it as a last resort that completely blocks the worker until a human is available. Same treatment for ralph. Next step: quick overview of the current prompting for this and some options.

**Decisions:**
- Add to built-in system prompt, not `.coven/system.md`
- Present as last resort (blocks worker until human available)
- Same approach for ralph
- `<break>` tag name is fine as-is

## Done
