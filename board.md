# Board

---

## P1: Review break and wait-for-user state

Report the current state of `<break>` and `<wait-for-user>` for both ralph and worker: where they're referenced in prompts and code, what behavior they produce, and identify possible issues. Removing `<wait-for-user>` from the worker may have been a mistake.

## P1: self_transition_review test doesn't trigger a review session

The main agent completes trivial tasks in a single session without self-transitioning for review. The test may need a harder task, or the prompt may need to better encourage review sessions.

## Done
