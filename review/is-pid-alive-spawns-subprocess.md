---
priority: P2
state: review
---

# `is_pid_alive` spawns subprocess instead of using syscall

`src/worker_state.rs:267-274` checks whether a PID is alive by spawning a subprocess:

```rust
fn is_pid_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}
```

This fork+execs `kill` for each worker state file during `read_all`, which runs on every dispatch cycle. The project already depends on `libc` (used in `session_loop.rs:558`), so `libc::kill` is available:

```rust
fn is_pid_alive(pid: u32) -> bool {
    // SAFETY: kill with signal 0 performs error checking without sending a signal.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}
```

This avoids fork+exec overhead and is the standard POSIX approach. The errno check for `ESRCH` (no such process) vs `EPERM` (process exists but owned by different user) could also be handled if needed, though for same-user worker processes the simpler `== 0` check suffices.

## Plan

Replace the subprocess-based `is_pid_alive` with a direct `libc::kill` syscall in `src/worker_state.rs`.

1. **Replace the function body** (lines 267-274): swap the `Command::new("kill")` implementation with `unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }`. Keep the existing doc comment. The `== 0` check is sufficient since coven workers always run as the same user.

2. **Remove unused import**: `Stdio` (line 15) is only used by the old `is_pid_alive`. Remove it from the `use std::process::{Command, Stdio}` line, leaving just `use std::process::Command;`.

3. **Run `cargo fmt`, `cargo clippy`, `cargo test`** to verify nothing breaks.
