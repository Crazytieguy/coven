---
priority: P1
state: new
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
