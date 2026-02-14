# Board

---

## P1: Test snapshots fail when run in wider terminal

Tests may fail when run in a real terminal because the display width is wider than in the non-interactive shell where Claude records snapshots. Investigate whether test code goes through display width logic and propose a fix.
## P1: Transition YAML parsing fails on colons in values

The `<next>` transition format uses YAML, which breaks when values contain colons (e.g. `task: Refine post-compaction context: system.md scope`). The model gets a poor error message and retries the same invalid syntax. Either improve the error message or switch to a more forgiving format.

## Done

- P1: Refine post-compaction context: system.md scope and dispatch faithfulness
- P1: Transition parsing failure behavior
- P1: Add "Done" section to board
- P1: Add main agent self-transition review test
- P1: Re-record VCR tests and fix snapshots
- P1: Improve post-compaction context loss
- P1: Input line splits on first keystroke during streaming
- P1: Pager keystroke capture in :N mode
