# Board

---

## P1: Bell sound: ring when `wait-for-user` is outputted

Recent bell fix overshot — should also ring when `wait-for-user` is outputted by ralph or worker (but no other states).

## P1: Support `wait-for-user` tag in `ralph`

The `wait-for-user` tag should also be supported by `ralph` if not already supported. Should share code with `worker`.

## Done

- P1: Bell sound: only ring when waiting for user input in run mode

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
