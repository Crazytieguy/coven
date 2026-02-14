# Board

---

## P1: Audit codebase for code smells

Attempt to identify code smells in the codebase — e.g. overly long functions, unclear naming, poor abstractions, dead code, inconsistent patterns. If the fix is obvious do it, if unclear post a question.

## P1: Audit codebase for error-prone duplication

Attempt to identify error-prone duplication in the codebase — e.g. repeated logic that could drift out of sync, copy-pasted patterns that should be shared, duplicated constants or config. If the fix is obvious do it, if unclear post a question.

## P1: Audit codebase for race conditions and concurrency issues

Attempt to identify race conditions and concurrency issues in the codebase — e.g. unsynchronized shared state, ordering assumptions, missing atomicity, async hazards. If the fix is obvious do it, if unclear post a question.

## P1: Audit codebase for error handling and edge case issues

Attempt to identify error handling and edge case issues in the codebase — e.g. swallowed errors, panics in non-panic contexts, missing validation at boundaries, unhandled None/Err cases. If the fix is obvious do it, if unclear post a question.

## P2: Investigate prompt issues causing flaky orchestration recordings

The `ambiguous_task` VCR recording is flaky — the main agent sometimes skips `land.sh` before transitioning and/or uses `<wait-for-user>` directly instead of transitioning to dispatch. The correct flow is: main adds questions to board → lands → transitions to dispatch → dispatch sleeps.

## P2: Reconsider wait-for-user abstraction

Is `wait-for-user` the right abstraction for both `worker` and `ralph`? Is it pulling its weight, or adding complexity and confusing the model?

## Done

- P1: Audit codebase for architectural issues
- P1: Verify spurious wakeup fix for race conditions and other wakeup sources
- P1: Investigate spurious worker wake-ups
- P1: Bell sound: recent fix overshot, should also ring when `wait-for-user` is outputted by ralph or worker (but no other states)
- P1: Bell sound: ring when `wait-for-user` is outputted (already works — both ralph and worker ring via `wait_for_interrupt_input`)

- P1: Bell sound: only ring when waiting for user input in run mode
- P1: Support `wait-for-user` tag in `ralph`
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
