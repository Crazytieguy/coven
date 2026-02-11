---
priority: P0
state: approved
---

# Find and fix VCR recording regressions

A recent change may have broken some VCR recordings. Run test cases with a short timeout and investigate any that take unexpectedly long — they likely indicate a regression where the recording gets stuck (e.g. waiting for input that never comes, a trigger that never fires, or a missing exit condition).

## Plan

1. Run `cargo run --bin record-vcr` with a short per-case timeout (e.g. 30-60s). Cases that hit the timeout are suspects.
2. For each stuck case, use the new progress output (from `vcr-recording-progress-output`) to identify where it hangs.
3. Investigate the root cause — likely a change to triggers, exit conditions, or command signatures that made existing test case configs stale.
4. Fix the issue (update test case config, fix the code regression, or both).
5. Re-record affected fixtures and run `cargo test` to verify.
