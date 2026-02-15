# Board

## P1: wait-for-user prompt final revision

Human feedback on the re-proposal: examples should be things that block **all** work, not just the work the agent is currently doing. Specifics:
- "A permission was denied" isn't the right trigger — many permission denials are fine. But "permission to run a critical workflow command" should trigger wait-for-user.
- "An error you can't resolve" — some errors should block and some shouldn't. Only errors that block all work qualify.
- No need to include the `sleep: true` note — that should be explained separately.
- Propose another final version with this principle in mind.

---

## Done
- P1: Split main into main + review agents
- P1: First typed character after entering interactive with Ctrl+O seems to be swallowed
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
- P1: wait-for-user re-proposal
