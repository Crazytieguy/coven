Issue: Orchestration next steps: CLAUDE.md integration (open design question), end-to-end testing
Status: draft

## Approach

This issue has two remaining items. I propose handling them as follows:

### 1. CLAUDE.md Integration — Keep Current Approach

The current interactive Y/n prompt in `coven init` is already implemented and works well. The design doc lists three concerns with automatic updates: (a) CLAUDE.md may not exist, (b) modifying existing content requires careful insertion, (c) user may prefer their own wording. The current code already handles all three cases — it creates CLAUDE.md if missing, appends if existing, and skips on decline.

**Recommendation: close this item as done.** The interactive prompt is the right UX — it's explicit, safe, and gives the user control. "Easy to miss" isn't a real concern because `coven init` is a one-time command the user actively runs, and the prompt is prominent.

### 2. End-to-End Worker Testing — Defer to P0

The P0 issue "VCR + snapshot testing for concurrent worker sessions" (`plans/concurrent-worker-testing.md`) already covers the testing infrastructure needed for the worker lifecycle. End-to-end testing of the orchestration flow (dispatch → agent → land) depends on that VCR infrastructure being in place first.

**Recommendation: close this item by deferring to the P0 issue.** Once concurrent VCR testing is implemented, single-worker end-to-end tests become a subset of that work.

### Resolution

If both recommendations are accepted, this issue can be removed from issues.md — CLAUDE.md integration is done, and testing is tracked by the P0 issue.

## Questions

### Should we close this tracking issue entirely?

The two remaining sub-items are either done (CLAUDE.md) or tracked elsewhere (P0 testing). Keeping this issue open adds no value. But if you want the orchestration tracking issue to stay open for other future items, we can keep it and just update the description.

Answer:

## Review

