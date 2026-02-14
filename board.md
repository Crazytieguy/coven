# Board

---

## P1: Bell sound: only ring when waiting for user input in run mode

`coven worker` and `coven ralph` should not play a bell every time Claude finishes a turn. Bell should only play when specifically waiting for user input: the only mode that does this at the end of a turn is `run`. Going idle doesn't need a bell/notification either. (Ideally a system notification rather than a sound, but the easier fix is restricting when bells play.)

**Findings:** 3 bell sites:
1. `session_loop.rs:440` — every time Claude finishes a turn (follow-up prompt)
2. `session_loop.rs:482` — after Ctrl+C interrupt, waiting for resume
3. `worker.rs:874` — worker goes idle/sleeping

(`renderer.rs:823` is an OSC terminator, not a bell.)

**Decisions:**
- Bell #1 (follow-up prompt): only play in `run` mode, not in `worker` or `ralph`
- Bell #2 (Ctrl+C interrupt): keep — user is waiting for input in `run`
- Bell #3 (worker idle): remove

## Done

- P1: Main agent should be more willing to ask clarifying questions
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
