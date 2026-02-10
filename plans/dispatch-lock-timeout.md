Issue: [P2] Add timeout to dispatch lock acquisition in worker_state.rs. Currently uses `fs2::FileExt` file locking with no timeout — could hang indefinitely if another process crashes while holding the lock.
Status: done

## Approach

Replace the blocking `file.lock_exclusive()` call in `acquire_dispatch_lock` with a polling loop using `file.try_lock_exclusive()`, sleeping between attempts up to a configurable timeout.

### Changes

**`src/worker_state.rs`** — modify `acquire_dispatch_lock`:

1. Change `lock_exclusive()` to a loop:
   - Call `try_lock_exclusive()`
   - If it succeeds, return the lock
   - If it fails with `WouldBlock`, sleep briefly and retry
   - If the elapsed time exceeds the timeout, return an error with a helpful message (e.g. "dispatch lock held for >30s — another worker may be stuck")
   - Any other error propagates immediately

2. Use `std::time::{Duration, Instant}` for timing. Poll interval: 100ms. Default timeout: 30s.

3. The timeout should be a constant in `worker_state.rs` (not user-configurable for now — it's an internal safeguard).

### Notes

- `fs2::FileExt::try_lock_exclusive()` returns `io::Error` with `ErrorKind::WouldBlock` when the lock is held.
- OS-level file locks are automatically released on process crash, so the "crashed while holding" scenario is mainly about zombie processes or stuck I/O. The timeout is a safety net.
- No changes needed to callers — the function signature stays the same (`Result<DispatchLock>`), it just errors after the timeout instead of blocking forever.

## Questions

None.

## Review

Not needed, I prefer manual human operator intervention. Instead, add a comment to the codebase that clarifies this preference. Do NOT add the loop
