Issue: [P2] Windows support: uses Unix-specific APIs (libc::tcflush, /dev/null, kill command, rsync). Need platform abstractions or #[cfg] guards to support x86_64-pc-windows-msvc target.
Status: draft

## Approach

Four Unix-specific call sites need platform abstractions:

### 1. `libc::tcflush` — `src/commands/session_loop.rs:492`

Used in `flush_stdin()` to discard buffered terminal input after the pager closes. Replace with `#[cfg]` branches:
- **Unix**: keep `libc::tcflush(STDIN_FILENO, TCIFLUSH)`
- **Windows**: use `crossterm::event::poll` in a loop with `Duration::ZERO` to drain pending events, or use `FlushConsoleInputBuffer` via the `windows-sys` crate. Crossterm is already a dependency, so the poll-drain approach is simpler.

### 2. `/dev/null` — `src/worker_state.rs:42`

`DispatchLock::from_recorded` opens `/dev/null` as a dummy file for VCR replay. Replace with:
- **Unix**: `/dev/null`
- **Windows**: `NUL`

Alternatively, since this is just a placeholder file handle for replay mode, consider using `tempfile` or just storing an `Option<File>` and making it `None` in replay mode (avoids the platform branch entirely).

### 3. `kill -0` — `src/worker_state.rs:231`

`is_pid_alive` shells out to `kill -0 <pid>` to check if a process exists. Replace with:
- **Unix**: keep `kill -0` or use `libc::kill(pid, 0)`
- **Windows**: use `OpenProcess` + `GetExitCodeProcess` via `windows-sys`, or use the `sysinfo` crate. A simpler option: use Rust's `std::process::Command` to run `tasklist /FI "PID eq <pid>"` — but that's slow. The `windows-sys` approach is more correct.

Alternatively, consider using `libc::kill(pid, 0)` on Unix (removes the `Command::new("kill")` shell-out) and a `#[cfg(windows)]` branch using `windows-sys`.

### 4. `rsync` — `src/worktree.rs:454`

`rsync_ignored` copies gitignored files from main worktree to a new worktree. Replace with:
- **Unix**: keep `rsync`
- **Windows**: implement a pure-Rust file copy using `std::fs::copy` in a loop over the file list. The rsync invocation is simple (`-a --files-from=-`), so a Rust reimplementation is straightforward: read the file list from the git command, iterate, create parent dirs, copy each file.

Alternatively, use `robocopy` on Windows, but a pure-Rust fallback is more portable and avoids another external dependency.

## Questions

### Should we add `windows-sys` as a dependency?

Using `windows-sys` gives clean access to `OpenProcess`/`GetExitCodeProcess` for PID checking and `FlushConsoleInputBuffer` for stdin flushing. The alternative is using crossterm (already a dep) for stdin flushing and `tasklist` for PID checking, which avoids a new dependency but is less clean.

Options:
- **Add `windows-sys`**: cleaner Windows APIs, but adds a dep that only matters on Windows
- **Avoid it**: use crossterm's poll-drain for stdin flushing, `tasklist` command for PID checking — no new deps but slightly hacky

Answer:

### Should we replace the `rsync` call with pure Rust on all platforms?

The rsync usage is simple enough to reimplement in Rust (`create dirs + copy files`). Doing so on all platforms would:
- Remove the rsync external dependency entirely
- Make the code consistent across platforms
- Slightly increase code but remove an external tool requirement

Alternatively, keep rsync on Unix and only use the Rust fallback on Windows.

Answer:

## Review

