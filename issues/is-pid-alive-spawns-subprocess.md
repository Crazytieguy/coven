---
priority: P2
state: new
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
