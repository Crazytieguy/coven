# Board

---

## P1: Refine post-compaction context: system.md scope and dispatch faithfulness

The recent post-compaction context loss fix was too aggressive. Two changes needed:
1. **system.md scope:** system.md should only include context useful to all agents (e.g. land.sh script, general workflow). Move agent-specific context out.
2. **Dispatch faithfulness:** The dispatch agent should copy useful context from the brief more faithfully, often copying content verbatim rather than summarizing.

## Done

- P1: Transition parsing failure behavior
- P1: Add "Done" section to board
- P1: Add main agent self-transition review test
- P1: Re-record VCR tests and fix snapshots
- P1: Improve post-compaction context loss
- P1: Input line splits on first keystroke during streaming
- P1: Pager keystroke capture in :N mode
