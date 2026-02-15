# Board

## P1: self_transition_review test doesn't trigger a review session

Tried a harder task (merge_intervals — sorting, merging overlapping/adjacent intervals, edge cases). Updated the fixture and re-recorded. Haiku still completes everything in one main session — it inlines the review rather than self-transitioning to a fresh context.

**Decisions:**
- Improve the prompt rather than dropping the requirement
- Explain the "why": a review with a fresh context window catches issues that could be missed — like fresh eyes
- Prefer explaining the "why" over ALL CAPS instructions
- Make the task slightly harder as well (safety buffer)

### Prompting options

Current text (lines 32-42 of main.md):
```
## Implementation Sessions

If more work remains, transition to main again to continue.

When implementation is complete, transition to main again for a **review session**: review the full diff (`git diff main...HEAD`), verify acceptance criteria, and fix anything that needs it.

When review passes:
1. Move the entry to the `## Done` section of `board.md` ...
```

**Option A — Explain the why, keep structure:**
Add a sentence after the review instruction:
```
When implementation is complete, transition to main again for a **review session** ...

A fresh context window acts like fresh eyes — it catches issues you'd overlook in the same session where you wrote the code.
```
Minimal change. Risk: Haiku may still optimize for efficiency since the structural cue ("transition to main") hasn't changed.

**Option B — Separate into two explicit phases:**
```
## Implementation Sessions

If more work remains, transition to main again to continue.

When implementation is complete, commit your work, update `scratch.md` with what you did, and transition to main for review. A fresh context window acts like fresh eyes — it catches issues you'd overlook in the same session where you wrote the code. Do not review or land in the same session as implementation.

## Review Sessions

Review the full diff (`git diff main...HEAD`), verify acceptance criteria, and fix anything that needs it.

When review passes:
1. Move the entry to the `## Done` section ...
```
Stronger structural signal — two H2 sections make it visually clear these are different phases. The explicit "do not" reinforces the boundary.

**Option C — Lifecycle list:**
```
## Session Lifecycle

Each main session does exactly one of:

1. **Implement** — write code, commit, update `scratch.md`. If more work remains, transition to main to continue. When implementation is complete, transition to main for review.
2. **Review** — review the full diff (`git diff main...HEAD`) with fresh eyes, verify acceptance criteria, fix issues. When review passes, move entry to Done, land, transition to dispatch.

A fresh context window acts like fresh eyes — it catches issues you'd overlook after a long implementation session. Never combine implement and review in one session.
```
Most explicit about the one-job-per-session constraint. Risk: slightly more prescriptive than the current tone.

**My lean:** Option B — it's the smallest structural change that creates a clear phase boundary while explaining the why. The separate H2 heading is a strong signal to models.

### Making the task harder

The merge_intervals function is straightforward enough that Haiku feels confident skipping review. Options:

- **Add a test requirement** — "Include unit tests in `tests/test_intervals.py`". Multiple files + testing = more surface area for review to catch issues.
- **Add complexity constraints** — "Handle intervals with negative numbers, floats, and single-point intervals like `[3, 3]`". More edge cases = more for review to verify.
- **Both** — tests + edge cases. This is my lean since it gives review something concrete to check.

**Questions:**
- Which prompting option (A/B/C)?
- How much harder to make the task?

---

## P1: Add wait-for-user to worker and ralph system prompts

Add `<wait-for-user>` to the built-in coven worker system prompt (not `.coven/system.md` — that's a template). Present it as a last resort that completely blocks the worker until a human is available. Same treatment for ralph. Next step: quick overview of the current prompting for this and some options.

**Decisions:**
- Add to built-in system prompt, not `.coven/system.md`
- Present as last resort (blocks worker until human available)
- Same approach for ralph
- `<break>` tag name is fine as-is

## Done
