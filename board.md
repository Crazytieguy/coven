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

## P1: wait-for-user re-proposal

**Proposal: shared prompt constant, calmer tone, drop "ambiguous requirements"**

**Prompt text** — new shared constant in `src/wait_for_user.rs` (or a `WAIT_FOR_USER_PROMPT` in an existing shared location):

```
`<wait-for-user>reason</wait-for-user>` — pauses your session until the user
responds. Your session is preserved and resumes with their input. Use when
you're blocked on something only a human can fix — a permission was denied,
an external service is down, or you've hit an error you can't resolve.
```

Worker appends: `Prefer \`sleep: true\` when work might become available later without human action.`

Ralph uses it as-is (break tag is the only other control, no need for extra guidance).

**Changes from current:**
- Drops "last resort", bold emphasis, "completely blocks" — just explains what it does
- Removes "fundamentally ambiguous requirements" as an example
- Keeps permission denied and unrecoverable error, adds "external service is down" as a concrete non-scary example

**Code sharing:**
- Extract prompt text to a shared constant both `transition.rs` and `ralph.rs` import
- Handling code stays separate — worker uses `Transition` enum + `run_phase_with_wait`, ralph uses direct `extract_tag_inner` + its own resume loop. Different enough that sharing would be forced.

**Questions:**
- Good to proceed?

---

## P1: First typed character after entering interactive with Ctrl+O seems to be swallowed

## Done
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
