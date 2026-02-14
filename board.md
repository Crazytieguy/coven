# Board

---

## P1: Improve post-compaction context loss

Move shared critical rules (like landing via `land.sh`) to `system.md` so they survive compaction. Don't be too aggressive — keep agent-specific workflow in agent prompts.

**Decisions:**
- Pursue proposal 1 (move critical rules to system.md) conservatively
- Don't be too aggressive with extraction — only truly shared invariants
- No other failure modes observed beyond `git push` and transition confusion

## P1: Transition parsing failure behavior

Enrich corrective prompt with available agents and bump auto-retries to 3. On final failure, agent should explicitly output wait-for-user tag before blocking on user input.

**Decisions:**
- Both proposals approved: enrich corrective prompt with real agent defs, and increase auto-retries
- Bump retries to 3 (up from proposed 2)
- Agent should explicitly output wait-for-user tag on final failure
- Was blocked on wait-for-user tag — now unblocked (implemented)

## Done

- P1: Add "Done" section to board
- P1: Add main agent self-transition review test
- P1: Re-record VCR tests and fix snapshots
