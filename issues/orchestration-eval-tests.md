---
priority: P1
state: new
---

# Design VCR tests that evaluate orchestration quality

The current orchestration tests (`worker_basic`, `concurrent_workers`) cover basic functionality. We need tests for harder scenarios that evaluate how well models pilot the orchestration system — e.g. correct dispatch decisions under competing priorities, proper state transitions when implementation fails, conflict resolution during landing, planning quality for ambiguous issues.

These tests double as evals: the VCR snapshots capture model behavior given the prompts and systems, so improving prompts/agents can be validated by re-recording and checking snapshot diffs.

Come up with several unique test scenarios that stress-test orchestration decision-making.
