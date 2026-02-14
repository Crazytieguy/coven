# Board

---

## P1: Transition parsing failure behavior

Enrich corrective prompt with available agents and bump auto-retries to 3. On final failure, agent should explicitly output wait-for-user tag before blocking on user input.

**Decisions:**
- Both proposals approved: enrich corrective prompt with real agent defs, and increase auto-retries
- Bump retries to 3 (up from proposed 2)
- Agent should explicitly output wait-for-user tag on final failure
- Was blocked on wait-for-user tag — now unblocked (implemented)

## P1: Input line splits on first keystroke during streaming

When typing during streaming, the first character appears on one line and the input then jumps to the next line, resulting in a visual split (e.g. "t" on one line, "te" on the next). Likely a rendering/cursor issue in the input display logic.

## P1: Pager keystroke capture in :N mode

Keystrokes in `:N` pager mode are captured by coven instead of the pager. Same root cause as Ctrl+O interactive sessions — apply same fix. Also investigate whether the first keystroke in Ctrl+O interactive mode fails to send (could be a Claude Code loading delay vs a coven issue).

## Done

- P1: Add "Done" section to board
- P1: Add main agent self-transition review test
- P1: Re-record VCR tests and fix snapshots
- P1: Improve post-compaction context loss
