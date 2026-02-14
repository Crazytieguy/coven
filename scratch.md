# Audit: Race Conditions and Concurrency Issues

## What I Did

Thorough review of all concurrency patterns in the codebase:
- Tokio async runtime, channels (mpsc, watch, bounded), `tokio::select!`
- Spawned tasks (terminal reader, fork multiplexers, stdout readers)
- File-based locking (semaphores, worker state)
- Terminal I/O coordination (pause/resume gate, tcflush)
- VCR context (RefCell, single-threaded by design)

## Issue Found and Fixed

**Non-atomic worker state writes** (`worker_state.rs:write_state`):
- `fs::write` opens with `O_TRUNC` (zeroing file) before writing content
- Concurrent `read_all` from another process could see empty/partial file
- JSON parse failure causes `read_all` to delete the "corrupt" file
- Fix: write to `.json.tmp` then rename (atomic on POSIX)

**Also fixed:** Removed duplicate `handle_inbound` in `event_loop.rs` from a broken merge by the other worker. It referenced nonexistent `SessionState` fields.

## Areas Reviewed (No Issues)

1. **Terminal reader pause/resume** (main.rs:136-164): Brief window where EventStream and child process could both read stdin. Well-mitigated by `drain_term_events()` + `tcflush()`. Inherent to cooperative approach.

2. **Fork multiplexer tasks** (fork.rs:82-88): Fire-and-forget `tokio::spawn`, but exit cleanly when channels close. No leak.

3. **Ref watcher TOCTOU** (worker.rs:855-868): Correctly avoided — watcher set up BEFORE baseline SHA is read.

4. **File semaphores** (semaphore.rs): OS-level exclusive locks via fs2. Polling with 100ms sleep is simple but correct. Lock released on drop (RAII).

5. **`tokio::select!` in `next_event`** (vcr.rs:397-410): Uses default random branch order. No bias.

6. **VCR RefCell** (vcr.rs): `!Send` by design — constrains to single-threaded LocalSet in `record_vcr`.

7. **Worker state `read_all` cleanup** (worker_state.rs:87-115): Read failures skip (no delete). Parse failures delete. After the atomic write fix, partial reads should not occur.

8. **`replace_event_channel`** (vcr.rs:415-420): Only called via `&mut Io`, so Rust borrow checker prevents concurrent access.

## What's Next

Review session is needed — check the full diff, verify acceptance criteria, then land.
