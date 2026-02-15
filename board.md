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

## Done
- P1: Split main into main + review agents
- P1: First typed character after entering interactive with Ctrl+O seems to be swallowed
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
