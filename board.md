# Board

---

## P1: Transition YAML parsing fails on colons in values

The `<next>` transition format uses YAML, which breaks when values contain colons (e.g. `task: Refine post-compaction context: system.md scope`). The model gets a poor error message and retries the same invalid syntax. Either improve the error message or switch to a more forgiving format.

## P1: Main agent should be more willing to ask clarifying questions

The main agent should be even more willing to ask clarifying questions instead of implementing. Propose several approaches to prompt changes, including at least one where the question flow becomes more first-class.

## Done

- P1: Refine post-compaction context: system.md scope and dispatch faithfulness
- P1: Transition parsing failure behavior
- P1: Add "Done" section to board
- P1: Add main agent self-transition review test
- P1: Re-record VCR tests and fix snapshots
- P1: Improve post-compaction context loss
- P1: Input line splits on first keystroke during streaming
- P1: Pager keystroke capture in :N mode
- P1: Test snapshots fail when run in wider terminal
