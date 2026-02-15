# Board

## P1: self_transition_review test doesn't trigger a review session

Prompt-only approach failed — even with explicit "Implementation Sessions" / "Review Sessions" H2s, "do not land during implementation", and a harder task (merge_intervals + unit tests), Haiku still inlines the review. The model reads the whole prompt and sees both phases as steps in one workflow.

**Proposal: split main into main + review agents**

The model can't skip what it doesn't know about. Instead of teaching one agent two modes, give each mode its own agent. The transition system already supports this — agents are just `.md` files.

**Lifecycle:** `dispatch → main × N → review → dispatch → sleep`

**main.md** — stripped of all landing/board-update responsibilities:
- Orient, Decide (post vs implement) stay as-is
- Implementation sessions: commit, write scratch.md, transition to main (more work) or review (done)
- Remove the review checklist entirely — main doesn't know how to land

**review.md** — new agent, single responsibility:
- Takes `task` arg (same as main)
- Reads scratch.md and the full diff (`git diff main...HEAD`)
- Verifies acceptance criteria, fixes issues if needed
- When it passes: update board → land → delete scratch.md → dispatch

**system.md** — update Lifecycle diagram

**init.rs** — add `REVIEW_PROMPT` constant + entry in `AGENT_TEMPLATES`

**Test fixture** — keep merge_intervals + unit tests task, re-record

**Questions:**
- Good to proceed with this split?

---

## P1: wait-for-user re-proposal

Current wait-for-user prompts (commit 050323a) are too aggressive and "ambiguous requirements" is a bad reason to wait-for-user — good workflows have built-in ways to ask async questions. Worker and ralph should share this prompt (and possibly code). Needs a new proposal.

### Context

**Worker** (`src/transition.rs`): Added "Wait for user (last resort)" section documenting `<wait-for-user>` alongside `<next>` and `sleep`. Examples: permission denied, fundamentally ambiguous requirements, unrecoverable error.

**Ralph** (`src/commands/ralph.rs`): Reworded existing `<wait-for-user>` docs with same last-resort framing.

### Task

Propose revised wait-for-user prompt wording — post to board for review before implementing. Also explore sharing the prompt text and/or code between worker and ralph.

## P1: First typed character after entering interactive with Ctrl+O seems to be swallowed

## Done
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
