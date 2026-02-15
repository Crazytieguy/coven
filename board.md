# Board

---

## P1: self_transition_review test doesn't trigger a review session

Tried a harder task (merge_intervals — sorting, merging overlapping/adjacent intervals, edge cases). Updated the fixture and re-recorded. Haiku still completes everything in one main session — it inlines the review rather than self-transitioning to a fresh context.

**Decisions:**
- Improve the prompt rather than dropping the requirement
- Explain the "why": a review with a fresh context window catches issues that could be missed — like fresh eyes
- Prefer explaining the "why" over ALL CAPS instructions
- Make the task slightly harder as well (safety buffer)
- Use Option B — separate into two explicit phases (Implementation Sessions / Review Sessions H2s)
- Try with just the prompt change first; if that's not enough, add a unit test requirement to the task

## Done
- P1: Add wait-for-user to worker and ralph system prompts
