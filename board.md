# Board

---

## P1: wait-for-user re-proposal

Revise the prompt text. The guiding principle: examples should be things that block **all** work, not just the agent's current task. Many permission denials are fine (the agent can work around them). "An external service is down" or "an error you can't resolve" may not block all work either. Focus on truly session-blocking situations.

Also: drop the `sleep: true` note from the worker append — that should be explained separately.

**Code sharing:** Extract prompt text to a shared constant both `transition.rs` and `ralph.rs` import. Handling code stays separate.

**Decisions:**
- Calmer tone, no "last resort" or bold emphasis — just explain what it does
- Drop "fundamentally ambiguous requirements" as an example
- Examples must be things that block ALL work, not just current task

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
