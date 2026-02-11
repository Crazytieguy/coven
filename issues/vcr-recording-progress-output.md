---
priority: P1
state: new
---

# Add progress output to VCR recording

`cargo run --bin record-vcr` runs silently — there's no way to tell what's happening during recording. This makes it painful to use from automated workers or when recording slow multi-step orchestration tests.

The recorder should stream some lightweight progress output so you can tell:
- Which test case is currently being recorded
- Which step (for multi-step tests) is active
- Key events as they happen (dispatch started, agent session started, landing, etc.)

This could use the coven display format or a simpler structured log. The key requirement is that a human or agent watching the output can tell whether recording is progressing or stuck.
