Issue: [P1] A single Ctrl+C Kills coven instead of just interrupting claude
Status: draft

## Approach

The problem has two facets: (1) `runner.kill()` sends SIGKILL, which is unnecessarily aggressive, and (2) coven has no OS-level SIGINT handler as defense-in-depth.

### 1. Graceful interrupt via SIGINT instead of SIGKILL

Currently `session_loop.rs` line 149 calls `runner.kill()` → `child.kill()` → SIGKILL. The claude CLI handles SIGINT gracefully (aborts current operation, emits a result). We should:

- Add a `runner.interrupt()` method that sends SIGINT to the child process (via `nix::sys::signal::kill(pid, Signal::SIGINT)` or raw `libc::kill`).
- In `session_loop.rs`, replace `runner.kill()` with `runner.interrupt()`.
- The session loop then continues processing events. Claude should respond to SIGINT by finishing up and emitting a result, which flows through normal completion handling.
- If claude doesn't respond within a reasonable timeout (e.g. 3s), fall back to `runner.kill()`.

This changes the Ctrl+C flow from "kill claude, offer resume" to "interrupt claude, it finishes gracefully, continue session." The `SessionOutcome::Interrupted` path in `run.rs`/`ralph.rs` would become a fallback for the timeout case only.

### 2. OS-level SIGINT handler (defense-in-depth)

Even though crossterm raw mode suppresses terminal SIGINT generation from Ctrl+C, external signals (e.g. `kill -INT`) can still reach coven. Currently coven dies with the default handler. We should:

- Install a SIGINT handler at startup (using `tokio::signal::unix::signal(SignalKind::interrupt())`) that converts the signal into an internal event (e.g. sent on the terminal event channel).
- This ensures that even if SIGINT reaches the process, it's handled gracefully instead of killing coven.
- The handler should be installed in `main.rs` before enabling raw mode.

### 3. Child process group isolation

Spawn the claude subprocess in its own process group (via `pre_exec` with `libc::setpgid(0, 0)`). This ensures that:
- If raw mode fails to suppress SIGINT, only coven receives it (not claude too)
- `runner.interrupt()` can target just the child process precisely
- No accidental double-signaling from both coven's handler and the terminal driver

### Files to change

- `src/session/runner.rs`: Add `interrupt()` method, add `pre_exec` for process group isolation
- `src/commands/session_loop.rs`: Use `interrupt()` + timeout instead of `kill()`
- `src/main.rs`: Install SIGINT handler, forward to event channel
- `src/vcr.rs` / `src/event.rs`: May need a new event variant for OS-level signals
- `Cargo.toml`: May need `libc` or `nix` crate for signal sending

### Implementation order

1. Add process group isolation to child spawn (smallest change, pure safety improvement)
2. Add `runner.interrupt()` method
3. Change session_loop to use interrupt + timeout
4. Add OS-level SIGINT handler in main.rs

## Questions

### Should Ctrl+C behavior differ between modes?

In `run` mode (interactive), Ctrl+C interrupts claude and you stay in coven — this is clearly the right UX. In `ralph` mode (looping), should Ctrl+C interrupt the current iteration or stop the loop? And in `worker` mode, Ctrl+C probably shouldn't affect the automated pipeline at all.

Current behavior: all modes kill claude and offer resume (run) or exit (ralph). Should we keep mode-specific differences, or should Ctrl+C always mean "interrupt claude, keep coven alive"?

Answer:

### How to handle the timeout fallback?

If we send SIGINT and claude doesn't respond within N seconds, we need to escalate to SIGKILL. Should we:
- (a) Use a fixed timeout (e.g. 3s) and silently escalate
- (b) Show the user a message like "claude not responding, force-killing..."
- (c) Require a second Ctrl+C to escalate (common shell pattern: first Ctrl+C = SIGINT, second = SIGKILL)

Option (c) is the most intuitive for terminal users. Option (a) is simplest.

Answer:

## Review

