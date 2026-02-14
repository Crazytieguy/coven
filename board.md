# Board

## P1: Main agent should be more willing to ask clarifying questions

Which variation of the pre-implementation checkpoint? Pick one:

**A: Soft first-session checkpoint** — First session *focuses on* understanding and planning, asks if questions arise, but may start coding if everything is clear.

**B: Hard first-session checkpoint** — First session is research-only, never writes code. Agent must either ask questions or write its plan in scratch.md and self-transition before any implementation.

**C: Mandatory question round-trip** — Agent always does one round-trip before implementing, even if just confirming understanding ("I plan to do X — any concerns?"). Guarantees human review of approach before code.

**Decisions:**
- Approach 3 chosen: pre-implementation checkpoint — prompt the agent to always spend its first session reading the task and listing questions before writing any code, a "plan then ask" phase built into the prompt structure
- Approaches 1, 2, and 4 not selected
- Agent should propose a few concrete variations of approach 3 for the user to choose from

---

## P1: Investigate bell sound behavior

Investigate when exactly coven plays a bell sound. The user is getting a lot of them and not sure they're always helpful.

## Done

- P1: Transition YAML parsing fails on colons in values
- P1: Refine post-compaction context: system.md scope and dispatch faithfulness
- P1: Transition parsing failure behavior
- P1: Add "Done" section to board
- P1: Add main agent self-transition review test
- P1: Re-record VCR tests and fix snapshots
- P1: Improve post-compaction context loss
- P1: Input line splits on first keystroke during streaming
- P1: Pager keystroke capture in :N mode
- P1: Test snapshots fail when run in wider terminal
