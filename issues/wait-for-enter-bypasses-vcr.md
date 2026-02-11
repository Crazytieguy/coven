---
priority: P0
state: approved
---

# `wait_for_enter_or_exit` bypasses VCR

`wait_for_enter_or_exit` in `src/commands/worker.rs:816-829` calls `io.next_event().await` directly instead of going through `vcr.call("next_event", ...)`.

This violates the project convention that every external I/O call must go through `vcr.call()` so it's recorded during recording and replayed deterministically during tests.

**Impact:** Any VCR test that exercises the worker retry paths (ff-retry limit exceeded, land error pause, conflict resolution pause) would hang during replay because `io.next_event()` blocks on an empty channel with no recorded data to replay. This is a blocker for orchestration testing.

**Affected code paths:**
- `handle_ff_retry` (worker.rs:554) — calls `wait_for_enter_or_exit` when `attempts > MAX_LAND_ATTEMPTS`
- `handle_land_error` (worker.rs:579) — calls `wait_for_enter_or_exit` after non-conflict land failure
- `handle_conflict` (worker.rs:596) — calls `wait_for_enter_or_exit` when conflict resolution exceeds `MAX_LAND_ATTEMPTS`

**Fix:** Change `wait_for_enter_or_exit` to accept a `&VcrContext` parameter and route through `vcr.call("next_event", ...)`, matching the pattern used in `wait_for_text_input` (session_loop.rs:454).

## Plan

### 1. Add `vcr: &VcrContext` parameter to `wait_for_enter_or_exit` (worker.rs:816)

Change the signature from:
```rust
async fn wait_for_enter_or_exit(io: &mut Io) -> Result<bool> {
```
to:
```rust
async fn wait_for_enter_or_exit(io: &mut Io, vcr: &VcrContext) -> Result<bool> {
```

### 2. Wrap `io.next_event()` in `vcr.call()` (worker.rs:818)

Replace:
```rust
let io_event = io.next_event().await?;
```
with:
```rust
let io_event: IoEvent = vcr
    .call("next_event", (), async |(): &()| io.next_event().await)
    .await?;
```

This matches the exact pattern used in `wait_for_text_input` (session_loop.rs:454-456) and `run_session_loop` (worker.rs:1120).

### 3. Update all three call sites to pass `ctx.vcr`

All callers already have access to `ctx: &mut PhaseContext<'_, W>` which contains `vcr: &VcrContext` (worker.rs:30).

- **worker.rs:564** (`handle_ff_retry`): `wait_for_enter_or_exit(ctx.io).await?` → `wait_for_enter_or_exit(ctx.io, ctx.vcr).await?`
- **worker.rs:588** (`handle_land_error`): same change
- **worker.rs:614** (`handle_conflict`): same change
