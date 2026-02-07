Blocks: The steering VCR test passes despite steering being broken — it only validates that stdin was written, not that Claude responded to the message. The test should verify that Claude's output actually changed in response to steering input. (Related to steering redesign — may be moot once steering is fixed, but worth noting for test integrity)

## What should happen with this test while steering is broken?

The steering test (`tests/cases/steering.toml`) sends "Actually, just count the lines instead" after tool 1, but the snapshot shows Claude responded with a summary (the original task), not a line count. The test passes because it only checks that the message was written to stdin.

Since steering is confirmed broken (`claude -p --input-format stream-json` ignores mid-stream stdin messages) and the steering feature itself is pending redesign (see questions/steering-redesign.md), there are several options:

1. **Make the test fail (document the bug)**: Fix the test assertion to verify Claude actually responded to the steering message. The test would then fail, clearly documenting that steering doesn't work. This is honest but means a known-failing test in the suite.

2. **Mark the test as ignored with a note**: Add `#[ignore]` (or equivalent in the test framework) with a comment explaining steering is broken and the test is deferred until the redesign. Remove the false-passing test rather than leave it as a misleading green check.

3. **Remove the test entirely**: Delete the steering test case since the feature doesn't work. Re-add a proper test when steering is redesigned. Simplest approach, but loses the test case as a reference.

4. **Rewrite to test current (broken) behavior**: Change the assertion to verify that steering is NOT acted on — i.e., assert that the output is a summary, not a line count. This documents the broken behavior explicitly. Feels wrong to "test" broken behavior though.

5. **Leave as-is until redesign**: The test is misleading but harmless. Once steering is redesigned, the test will be rewritten anyway. Avoids churn on a doomed test.

Answer:
