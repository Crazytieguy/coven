# Board

---

## P1: Verify spurious wakeup fix for race conditions and other wakeup sources

Make sure the spurious wake up fix doesn't cause a race condition. Also check if there might be other spurious wakeups: For instance I think maybe workers are woken up when another worktree is removed.

## P1: Audit codebase for architectural issues

Attempt to identify architectural issues in the codebase — e.g. misplaced responsibilities, overly coupled modules, unclear boundaries between components. If the fix is obvious do it, if unclear post a question.

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
