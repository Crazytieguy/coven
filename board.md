# Board

## P1: self_transition_review test doesn't trigger a review session

Tried a harder task (merge_intervals — sorting, merging overlapping/adjacent intervals, edge cases). Updated the fixture and re-recorded. Haiku still completes everything in one main session:

1. Reads board + code
2. Implements merge_intervals (28 lines)
3. Commits
4. Runs `git diff main...HEAD` and verifies acceptance criteria inline
5. Updates board to Done
6. Transitions to dispatch

The model does a review, but it inlines it into the implementation session rather than self-transitioning to a new main session. Making the task harder doesn't change this — haiku treats "implement then review" as a single workflow.

The fixture is updated and snapshot accepted (test passes). Two options:

1. **Prompt change** — make the self-transition instruction stronger/more explicit, e.g. "You MUST transition to main for review — never review in the same session you implemented in"
2. **Drop the requirement** — accept that inline review is good enough and adjust the test expectations accordingly

**Questions:**
- Which direction? Stronger prompt, or accept inline review?

---

## P1: Add wait-for-user to worker and ralph system prompts

Add `<wait-for-user>` to the built-in coven worker system prompt (not `.coven/system.md` — that's a template). Present it as a last resort that completely blocks the worker until a human is available. Same treatment for ralph. Next step: quick overview of the current prompting for this and some options.

**Decisions:**
- Add to built-in system prompt, not `.coven/system.md`
- Present as last resort (blocks worker until human available)
- Same approach for ralph
- `<break>` tag name is fine as-is

## Done
