# Board

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

## P1: Split main into main + review agents

Prompt-only approach failed — Haiku inlines the review. Split into separate agents so the model can't skip what it doesn't know about.

**Lifecycle:** `dispatch → main × N → review → dispatch → sleep`

**main.md** — stripped of all landing/board-update responsibilities:
- Orient, Decide (post vs implement) stay as-is
- Implementation sessions: commit, write scratch.md, transition to main (more work) or review (done)
- Remove the review checklist entirely — main doesn't know how to land

**review.md** — new agent, single responsibility:
- Takes `task` arg (same as main)
- Reads the original board issue + scratch.md + full diff (`git diff main...HEAD`)
- Judges whether to land or post questions: if main made decisions without asking, discard work and post questions on the board instead
- Evaluates implementation quality and improves anything noticeable
- When it passes: update board → land → delete scratch.md → dispatch

**Also:** system.md (lifecycle diagram), init.rs (`REVIEW_PROMPT` + `AGENT_TEMPLATES`), test fixture (re-record)

**Decisions:**
- Approved: split main into main + review agents
- Review agent reads the original board issue and makes its own judgement on landing vs posting questions
- If main made decisions without asking: discard work and post questions on the board
- Review agent evaluates quality and improves what it can

---

## Done
- P1: First typed character after entering interactive with Ctrl+O seems to be swallowed
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
