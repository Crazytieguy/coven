---
priority: P1
state: review
---

# Investigate interrupt/resume session producing immediate error

When interrupting a session mid-stream and sending "Continue where you left off", the resumed session shows `Error  $0.00 · 0.0s · 0 turns` — zero cost, zero time, zero turns. The resumed session immediately fails with no work done.

## Expected Behavior

The resumed session should continue working from where it was interrupted, or at least produce a meaningful error message.

## Steps to Reproduce

1. Start a session that triggers tool use (e.g., reading a file)
2. Interrupt mid-stream
3. Send a follow-up message like "Continue where you left off"
4. Observe the second session fails immediately

## Context

- Snapshot: `tests/cases/session/interrupt_resume/interrupt_resume.snap` (lines 13-14 in current snapshot)
- VCR fixture: `tests/cases/session/interrupt_resume/interrupt_resume.vcr`
- Found during snapshot audit (issue `audit-snapshot-tests.md`, finding M2)
- Could be a transient issue — re-recording may help diagnose
- May be related to session ID handling or timing of the interrupt

## Plan

**Recommendation: Close as false positive.** The reported error does not exist.

### Investigation Findings

The audit finding M2 that created this issue claimed lines 13-14 of `interrupt_resume.snap` showed `Error  $0.00 · 0.0s · 0 turns`. This was a hallucination by the audit model. The evidence:

1. **Current snapshot** shows a successful resume: `[3] Thinking...` → model response → `Done  $0.01 · 3.6s · 1 turn`
2. **Pre-audit snapshot** (before commit `10101d8`) shows the same successful resume — the only change in that commit was fixing carriage-return rendering on line 12 (the typing display), not fixing any error
3. **VCR fixture** confirms `"subtype":"success"`, `"is_error":false`, `"num_turns":1`, `"total_cost_usd":0.00947055` — no error whatsoever
4. **Git history** shows no prior version of the snapshot with an `Error` status line

The interrupt/resume flow works correctly: the first session is killed, a new session spawns with `--resume <session_id>`, the model picks up context from the previous session (message numbering continues at `[3]`), and completes successfully.

### Action

Delete this issue file — no code changes needed
