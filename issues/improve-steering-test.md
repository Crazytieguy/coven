---
priority: P2
state: new
---

# Improve steering test to verify model follows redirection

The current steering test (`steering.vcr`) sends a mid-stream steering instruction ("Actually, just count the lines in each file instead") but the model ignores it and continues with the original task (file summaries).

The steering arrives during a `content_block_start` (tool use) trigger, which means it likely reaches the model after the most recent batch of tool calls has already been submitted. By the time the model sees the steering, it's committed to its current plan.

## Desired Outcome

The steering test should demonstrate that mid-stream steering actually redirects the model's behavior. This requires:

1. A task that takes multiple tool call batches (not just one batch of reads)
2. Steering timed to arrive between batches, so the model can process it before committing to the next batch

## Context

- Snapshot: `tests/cases/session/steering/steering.snap`
- VCR fixture: `tests/cases/session/steering/steering.vcr`
- Found during snapshot audit (issue `audit-snapshot-tests.md`, finding M1)
