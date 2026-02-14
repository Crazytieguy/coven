# Reconsider wait-for-user abstraction

## Session 1: Analysis

### How `<wait-for-user>` works

Two separate implementations share the tag name:

**Worker** (transition.rs + worker.rs): `<wait-for-user>` is a `Transition` enum variant alongside `Next` and `Sleep`. It's documented in the transition system prompt injected into every agent session. When an agent outputs it, `run_phase_with_wait` shows the reason, rings the bell, waits for user input, then resumes the same session.

**Ralph** (ralph.rs): `<wait-for-user>` is checked directly via `extract_tag_inner` in `handle_session_outcome`, before the break tag. Same behavior: show reason, ring bell, wait, resume.

### Where it adds complexity

1. `Transition` enum has 3 variants instead of 2
2. Transition system prompt has an extra section documenting it
3. Corrective prompt (final retry) references it as a fallback
4. `run_phase_with_wait` wraps `run_phase_session` with a WaitForUser retry loop
5. `run_agent_chain` has a `WaitForUser` arm that panics ("unexpected")
6. Ralph's `handle_session_outcome` has additional logic before the break-tag check

### The model confusion problem

The other board issue notes: "the main agent sometimes uses `<wait-for-user>` directly instead of transitioning to dispatch." In the orchestration flow, when an agent has questions, the correct path is:
- Add questions to board → land → transition to dispatch → dispatch sleeps

`<wait-for-user>` gives the model an easier alternative that bypasses this flow. It's a shortcut that undermines the board mechanism.

### Where `<wait-for-user>` is valuable

**Ralph**: It's the ONLY mechanism for pausing the loop while preserving session context. Without it, the model would need to `<break>` (ending the loop) and the user would have to restart ralph from scratch. No alternative exists.

**Worker — permission denied**: If a command is denied, the agent can immediately tell the user and resume after the fix. But the existing fallback path (transition parse failure → auto-retry → manual user input via `wait_for_transition_input`) already handles this case, just less gracefully.

**Worker — concurrent conflicts**: In the `concurrent_workers` test, a worker discovers its task was already done and uses `<wait-for-user>` to ask what to do. But the ideal behavior would be: note on board, land, transition to dispatch.

### Key question

In worker, `<wait-for-user>` competes with the board flow. In ralph, it's the only option. Should we:

- **(A)** Remove from worker, keep in ralph — simplifies worker, fixes model confusion, ralph keeps its only input mechanism
- **(B)** Keep in both but refine prompts to reduce misuse — preserves the "permission denied" use case in worker
- **(C)** Remove from both — ralph would need `<break>` for pauses (loses session continuity)

## Next

Waiting for decision on the approach before implementing.
