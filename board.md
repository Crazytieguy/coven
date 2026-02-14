# Board

## P1: Investigate bell sound behavior

Investigate when exactly coven plays a bell sound. The user is getting a lot of them and not sure they're always helpful.

**Findings:** 3 bell sites:
1. `session_loop.rs:440` — every time Claude finishes a turn (follow-up prompt)
2. `session_loop.rs:482` — after Ctrl+C interrupt, waiting for resume
3. `worker.rs:874` — worker goes idle/sleeping

(`renderer.rs:823` is an OSC terminator, not a bell.)

**Questions:**
- Which of these bells are useful vs annoying? Should any be removed?
- Should there be a `--no-bell` flag, or should we just change the defaults?

---

## P1: Main agent should be more willing to ask clarifying questions

First session *focuses on* understanding and planning, asks if questions arise, but may start coding if everything is clear.

**Decisions:**
- Approach 3 chosen: pre-implementation checkpoint — prompt the agent to always spend its first session reading the task and listing questions before writing any code, a "plan then ask" phase built into the prompt structure
- Approaches 1, 2, and 4 not selected
- Variation A (soft first-session checkpoint) chosen: first session focuses on understanding and planning, asks if questions arise, but may start coding if everything is clear

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
