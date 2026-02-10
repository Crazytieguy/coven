Issue: [P0] `runner.kill()` uses SIGKILL which doesn't give Claude time to persist the conversation. Resume after Ctrl+C fails with "No conversation found with session ID". Consider using SIGTERM or closing stdin instead.
Status: draft

## Approach

### Problem

`runner.kill()` calls `tokio::process::Child::kill()` which sends SIGKILL. The Claude CLI can't catch SIGKILL, so it dies without persisting the conversation. When coven later tries `--resume <session_id>`, Claude reports "No conversation found."

### Fix

Replace `runner.kill()` with a new `runner.interrupt()` method that:

1. Sends SIGINT to the Claude process (via `libc::kill(pid, SIGINT)`)
2. Races `child.wait()` against a 3-second timeout
3. If the timeout expires, falls back to SIGKILL via `child.kill()`

SIGINT is the right signal because it's what a terminal Ctrl+C would normally send. Claude CLI is designed to handle it — saving state, flushing output, and exiting cleanly.

### Changes

**`src/session/runner.rs`:**
- Add `pub async fn interrupt(&mut self) -> Result<()>` method
- Implementation: get PID from `child.id()`, send SIGINT via `libc::kill`, then `tokio::time::timeout(Duration::from_secs(3), child.wait())`, fallback to `child.kill()` on timeout
- Keep `kill()` around for any future hard-kill needs, but it won't be called from the Ctrl+C path

**`src/commands/session_loop.rs`:**
- Change the `InputAction::Interrupt` handler from `runner.kill()` to `runner.interrupt()`

**VCR considerations:**
- `interrupt()` uses `child.id()` and `libc::kill()` which operate on the real process, not through VCR. During VCR replay there's no real child process. The method should no-op when `self.child` is `None` (stub mode), matching `kill()`'s existing guard.

**Dependencies:**
- `libc` is already a transitive dependency; add it as a direct dependency if not already present

## Questions

### Should we also send SIGINT to the process group?

Claude CLI may spawn child processes. Sending SIGINT to just the parent PID means children might linger. We could use `libc::kill(-pid, SIGINT)` to signal the entire process group, but only if Claude is in its own process group (which it is by default as a child process via `tokio::process::Command`).

Actually, child processes spawned by `tokio::process::Command` inherit the parent's process group by default, so `-pid` would also signal coven itself. To use process-group signaling safely, we'd need to spawn Claude in its own process group via `pre_exec(|| { libc::setpgid(0, 0); })`.

For now, signaling just the parent PID is simpler and sufficient — Claude CLI should propagate signals to its children internally.

Answer:

### Timeout duration

3 seconds seems reasonable for Claude to persist a conversation. Too short and we SIGKILL anyway; too long and Ctrl+C feels unresponsive. Is 3 seconds right?

Answer:

## Review
