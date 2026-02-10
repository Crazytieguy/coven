Issue: [P2] Create a features / intended behavior reference document so it's clear what each command mode supports (interaction modes, session lifecycle, rendering expectations). Makes it easier to verify whether code reflects intended behavior.
Status: draft

## Approach

Create `docs/features.md` — a concise reference document covering each command mode's intended behavior. This serves as a spec that tests and code can be verified against.

### Structure

```
# Features Reference

## Command Modes

### Run (default)
- Purpose, invocation
- Session lifecycle: Starting → Running → WaitingForInput → follow-up or exit
- Interaction: steering (Enter), follow-ups (Alt+Enter), :N inspection, Ctrl+C, Ctrl+D, Escape

### Ralph
- Purpose, invocation, key flags (--iterations, --break-tag, --no-break)
- Loop lifecycle: iteration header → spawn session → run → check break tag → next
- System prompt injection behavior
- Interruption and resume within iterations
- Cost tracking across iterations

### Worker
- Purpose, invocation, key flags (--branch, --worktree-base)
- Dispatch → Agent → Land lifecycle
- Worktree management, state synchronization
- Conflict resolution (rebase retry, agent resume, max attempts)
- Permission defaults

### Auxiliary (init, status, gc)
- Brief description of each

## Interaction Model
- Table: mode × capability (steering, follow-ups, multi-session, looping, conflict handling)
- Event buffering behavior during input
- Session state machine (shared across modes)

## Rendering
- One line per tool call, streaming text, collapsed/inline thinking
- Message numbering and :N pager
- Session metadata display (id, model, cost, time, turns)
- Subagent tracking
- Terminal features (raw mode, help overlay, turn separators, title updates)
```

### Scope

- Document current behavior as implemented, not aspirational features
- Keep it factual and concise — a reference, not a tutorial
- Cross-reference README.md for user-facing docs; this is the internal spec

## Questions

None.

## Review

