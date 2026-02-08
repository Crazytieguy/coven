Issue: The steering VCR test passes despite steering being broken — it only validates that stdin was written, not that Claude responded to the message. The test should verify that Claude's output actually changed in response to steering input.
Status: rejected

## Approach

The steering VCR test currently validates two things:

1. `validate_vcr` checks that the steering message appears in the VCR stdin lines
2. The snapshot matches the rendered display

But neither of these validates that Claude _responded_ to the steering message. The current VCR recording actually proves Claude ignored it — thinking "Simple file. Let me summarize it." and outputting a summary, not a line count.

### Option A: Mark the test as expected-broken

Add a comment to the steering snapshot acknowledging that the output shows the original task (summary) rather than the steered task (line count). This makes the test honest about what it's testing — the display of a steering attempt, not successful steering.

No code changes needed beyond updating the snapshot comment. Once the steering redesign lands, re-recording the VCR will produce a response that actually follows the steering message, and the snapshot will update naturally.

### Option B: Add assertion that output reflects steering

Add a check in the test that the rendered output contains evidence of the steered task (e.g., "2 lines" or "line count") rather than the original task. This would make the test _fail_ until steering actually works. We'd need to `#[ignore]` or `#[should_panic]` it in the meantime.

### Recommendation

Option A is more practical — the test still serves a purpose (validating display rendering for multi-message sessions), and the steering-redesign plan will fix the underlying issue. Adding a comment makes the situation explicit without losing test coverage.

## Questions

### Should the test fail now to signal steering is broken, or pass while documenting the limitation?

Option A (recommended): Keep the test passing. Add a comment to the test case TOML and/or snapshot noting that the current VCR recording shows Claude ignoring the steering message, and that the test validates display rendering only — not steering effectiveness. Once the steering redesign lands and the VCR is re-recorded, the comment can be removed.

Option B: Make the test fail (via `#[ignore]` or an explicit assertion) to create pressure to fix steering. This is more principled but loses test coverage for display rendering in the meantime.

Answer:

## Review

Steering isn't broken, we just need to make the test use a longer multi step task so we can see it in action.
