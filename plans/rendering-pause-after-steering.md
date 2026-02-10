Issue: [P0] I think sometimes rendering pauses after a steering message and there's no way to resume it
Status: draft

## Approach

### Root cause analysis

There are two possible mechanisms that could cause rendering to pause after a steering message:

**Mechanism 1: `send_message` blocks the event loop**

`send_message` (stdin write + flush) is awaited inline in the main event loop (`session_loop.rs:120-123`). While this await is pending, no events are processed — no Claude output rendered, no terminal input read. If the Claude process is slow to consume stdin (e.g., busy writing output), the pipe buffer fills and the write blocks.

In practice, single-message writes are small enough that this is unlikely to be the primary cause, but it's a latent issue.

**Mechanism 2: Claude Code doesn't respond to stdin mid-stream**

Claude Code's `-p --output-format stream-json` mode may not reliably handle stdin messages written during active streaming. The process might:
- Buffer the stdin input and never act on it (treats it as a follow-up for after the current turn, but the current turn's Result event has already been sent)
- Silently ignore the input
- Read the input but not produce new output

If Claude Code reads the steering text but doesn't produce any new streaming events, coven's event loop sits at `io.next_event().await` indefinitely — rendering appears frozen. The terminal channel is still alive so keyboard input works, but there's no Claude output to display, and the user sees no activity.

### Proposed fix

**Phase 1: Non-blocking `send_message`** (safe, addresses Mechanism 1)

Move the `send_message` call out of the blocking path of the event loop. Instead of awaiting inline:

```rust
// Before (blocks event loop):
vcr.call("send_message", text, async |t: &String| {
    runner.send_message(t).await
}).await?;
```

Use `tokio::spawn` or a dedicated write channel so the event loop continues processing events while the write happens in the background. The VCR wrapping complicates this — may need a "fire-and-forget" VCR call variant, or just accept that steering isn't VCR-recorded (it's a write, not a read).

**Phase 2: Steering timeout** (addresses Mechanism 2)

After sending a steering message, start a timeout. If no new Claude events arrive within N seconds (e.g., 10s), display a status message like "No response to steering message — Claude may not have received it." This at least unblocks the user's mental model.

**Phase 3: Investigate Claude Code's stdin behavior** (diagnostic)

Test what actually happens when you write to Claude Code's stdin during streaming vs. after a Result. Document the behavior so we can decide if steering needs to be redesigned (e.g., as a follow-up after the current turn rather than a mid-stream write).

## Questions

### Should we make `send_message` non-blocking for follow-ups too?

Follow-up sends happen at line 129-132 (from InputAction::Submit in FollowUp mode) and at line 197-200 (from process_claude_event when draining pending_followups). Making these non-blocking would be more consistent, but follow-ups are sent when Claude has finished (Result received), so blocking is less risky since no streaming events are expected during the write.

Answer:

### Should the timeout trigger an automatic retry or just a status message?

Options:
- Just show a status message (informational, low risk)
- Resend the steering message as a proper follow-up (might cause duplicate input)
- Close stdin and let the session end (destructive but unblocks)

Answer:

### What's the right timeout duration?

Claude can take 10-30+ seconds to "think" before producing output. A short timeout would false-positive; a long timeout delays feedback. Suggestion: 15 seconds with a status update, not an action.

Answer:

## Review
