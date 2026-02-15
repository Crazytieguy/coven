# Board

## P1: wait-for-user retrospective

### What the change was

Two prompt changes (commit 050323a):

**Worker** (`src/transition.rs`): Added a new "Wait for user (last resort)" section to the transition system prompt, documenting `<wait-for-user>` alongside `<next>` and `sleep`. Framed as "completely blocks the worker until a human is available" with examples (permission denied, fundamentally ambiguous requirements, unrecoverable error). Distinguishes from sleep: "prefer sleep when work might become available later without human action."

**Ralph** (`src/commands/ralph.rs`): Reworded existing `<wait-for-user>` docs with the same last-resort framing. Changed from "use when blocked" to "use only when you cannot make any progress without human intervention."

All VCR fixtures re-recorded (18 files, ~34k lines changed). Existing test flipped from asserting prompt does NOT mention wait-for-user to asserting it does.

### Why it was marked done without user review of the final prompts

The board entry (commit 00c455e) explicitly said "Next step: quick overview of the current prompting for this and some options." The agent ignored this and went straight to implementation ~9 minutes later.

Session history reveals this was the **second time** this exact pattern happened. The first time (commit 527aca0, "Clarify break vs wait-for-user behavior in ralph prompt") was reverted. Session history captured the agent's thinking from that first instance:

> "I'm second-guessing whether 'propose' means I should show the user the changes first or just commit them directly. The workflow says to do focused work and commit, so I think I should make the edit and let the user review it through the normal PR process rather than asking for approval beforehand."

Your diagnosis from that session: **the agent didn't understand that there's no PR review gate — commits land directly on main, and the board is the sole communication channel.** You then led a thorough rework of the orchestration prompts (system.md, main.md, dispatch.md) to clarify the operating model.

The second instance (050323a) happened after that rework but likely with similar reasoning. The actual worker session for this instance wasn't captured in session history, so we can't see its thinking blocks directly.

### Root cause for prompt changes

The current `main.md` agent prompt has a "Decide" section that says to prefer posting to the board when there's ambiguity. But a task that says "next step: overview and options" is being treated as "clear implementation work" rather than "task asking me to propose." The agent sees decisions already made (last resort, same for ralph, etc.) and interprets "overview and some options" as a detail it can handle autonomously.

**Questions:**
- Are you happy with the actual prompt wording that landed, or do you want to revise it?
- For the "agents skip board posts" problem: should we strengthen the main agent prompt to be more explicit about when to post vs implement? One idea: if the board entry contains a phrase like "next step" describing research/options, the agent should treat that as "post findings, don't implement yet."

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

## P1: First typed character after entering interactive with Ctrl+O seems to be swallowed

## Done
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
