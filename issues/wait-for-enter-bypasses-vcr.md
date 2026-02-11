---
priority: P1
state: new
---

# `wait_for_enter_or_exit` bypasses VCR

`wait_for_enter_or_exit` in `src/commands/worker.rs:816-829` calls `io.next_event().await` directly instead of going through `vcr.call("next_event", ...)`.

This violates the project convention that every external I/O call must go through `vcr.call()` so it's recorded during recording and replayed deterministically during tests.

**Impact:** Any VCR test that exercises the worker retry paths (ff-retry limit exceeded, land error pause, conflict resolution pause) would hang during replay because `io.next_event()` blocks on an empty channel with no recorded data to replay.

**Affected code paths:**
- `handle_ff_retry` (worker.rs:554) — calls `wait_for_enter_or_exit` when `attempts > MAX_LAND_ATTEMPTS`
- `handle_land_error` (worker.rs:579) — calls `wait_for_enter_or_exit` after non-conflict land failure
- `handle_conflict` (worker.rs:596) — calls `wait_for_enter_or_exit` when conflict resolution exceeds `MAX_LAND_ATTEMPTS`

**Fix:** Change `wait_for_enter_or_exit` to accept a `&VcrContext` parameter and route through `vcr.call("next_event", ...)`, matching the pattern used in `wait_for_text_input` (session_loop.rs:454).
