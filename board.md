# Board

## P1: Main agent should be more willing to ask clarifying questions

The main agent should be even more willing to ask clarifying questions instead of implementing. Propose several approaches to prompt changes, including at least one where the question flow becomes more first-class.

**Approaches:**

1. **Prompt-only: strengthen existing language** — Move the Questions section earlier in main.md, add concrete trigger examples (e.g. "if you'd write 'I went with X' in a scratch note, ask first"), frame asking as the default for anything ambiguous.

2. **Lean into `<wait-for-user>` for questions** — Change the question flow to use `<wait-for-user>` directly instead of the board round-trip. Agent asks inline, gets an answer, continues working. Record decisions on the board after the fact. Much lower friction than land→dispatch→brief→dispatch→main.

3. **Pre-implementation checkpoint** — Prompt the agent to always spend its first session reading the task and listing questions before writing any code. A "plan then ask" phase built into the prompt structure.

4. **New `<ask>` tag (first-class)** — A dedicated transition tag for questions. Agent outputs `<ask>`, the system records the questions on the board, waits for user input, and injects the answer back. Combines board persistence with inline flow.

**Questions:**
- Which approach(es) to pursue? They're composable — e.g. 1+2, 1+3, or 1+4.
- For approach 2 vs 4: is board persistence of Q&A important, or is the conversation history sufficient?

---

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
