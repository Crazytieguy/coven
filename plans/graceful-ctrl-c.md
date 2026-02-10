Issue: [P0] `runner.kill()` uses SIGKILL which doesn't give Claude time to persist the conversation. Resume after Ctrl+C fails with "No conversation found with session ID". Consider using SIGTERM or closing stdin instead.
Status: draft

## Approach

### Problem

`runner.kill()` calls `tokio::process::Child::kill()` which sends SIGKILL. The Claude CLI can't catch SIGKILL, so it dies without persisting the conversation. When coven later tries `--resume <session_id>`, Claude reports "No conversation found."

### Key finding from testing

SIGINT to `claude -p` during generation doesn't cause immediate exit. Instead, Claude queues the exit and finishes the current model response first (observed to take up to ~30 seconds). It does eventually exit cleanly, just not quickly.

This means the original plan (SIGINT + 3-second timeout + SIGKILL fallback) would behave identically to today's SIGKILL in the common case (Ctrl+C during generation), since the 3-second timeout would expire while Claude is still finishing its response.

### Revised approach

Use a generous timeout so Claude has time to finish its response and persist the conversation. The tradeoff is responsiveness vs. data preservation — but the whole point of this issue is that we're losing conversations, so we should err on the patient side.

1. Send SIGINT to the Claude process
2. Show the user a status message like "Stopping Claude..." so they know it's working, not hung
3. Wait up to **30 seconds** for Claude to exit (long enough for a typical generation to complete)
4. If timeout expires, fall back to SIGKILL (at this point something is genuinely stuck)
5. A second Ctrl+C during the wait period should immediately SIGKILL (escape hatch for impatient users)

### Changes

**`src/session/runner.rs`:**
- Add `pub async fn interrupt(&mut self) -> Result<bool>` — returns `true` if Claude exited cleanly, `false` if we had to SIGKILL
- Implementation: get PID from `child.id()`, send SIGINT via `libc::kill`, then `tokio::time::timeout(Duration::from_secs(30), child.wait())`, fallback to `child.kill()` on timeout
- Keep `kill()` for the hard-kill path

**`src/commands/session_loop.rs`:**
- Change the `InputAction::Interrupt` handler to call `runner.interrupt()` instead of `runner.kill()`
- Show "Stopping Claude..." status while waiting
- Track whether we're already in the "stopping" state — if a second Ctrl+C arrives during the wait, call `runner.kill()` immediately

**VCR considerations:**
- Same as before: `interrupt()` should no-op when `self.child` is `None` (stub/replay mode)

**Dependencies:**
- `libc` — already a transitive dependency, add as direct if needed

## Questions

### Is 30 seconds the right timeout?

The observation was "half a minute" for a long generation. 30 seconds covers most cases, but a very long response could exceed it. Options:
- 30 seconds (covers most cases, still feels responsive with the status message)
- 60 seconds (covers nearly all cases, but feels long even with feedback)
- No timeout (always wait for clean exit, second Ctrl+C is the only escape hatch)

Answer:

### Should we close stdin in addition to SIGINT?

Closing stdin might signal Claude to stop accepting input and wrap up sooner. Or it might have no effect during generation. Worth testing, but we could also just start with SIGINT and add stdin-closing later if needed.

Answer:

## Review

