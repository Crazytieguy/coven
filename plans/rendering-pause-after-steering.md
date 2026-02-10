Issue: [P0] I think sometimes rendering pauses after a steering message and there's no way to resume it
Status: draft

## Approach

### Root cause: flush_event_buffer doesn't propagate session outcomes

This is almost certainly a symptom of the flush_event_buffer bug (tracked separately as the [P0] flush_event_buffer issue). Here's the specific scenario:

1. Claude is running, user starts typing (input activates, events get buffered)
2. Claude sends a Result event while the user is typing (gets buffered)
3. User presses Enter to send steering message
4. `input.handle_key()` clears the input line and deactivates input
5. `flush_event_buffer()` runs — renders the buffered Result event, but:
   - Passes `has_pending_followups: false`, so the "Done" line is shown
   - Sets `state.status = WaitingForInput`
   - **Does not return a SessionOutcome** (returns `()`)
6. Code continues to line 119: `renderer.render_steering_sent(&text)`
7. Code continues to line 120-123: `vcr.call("send_message", ...)` — writes to Claude's stdin
8. Claude's process may accept the message but never produce new output (it already completed its turn), or the write may fail silently
9. The loop returns to `io.next_event().await` — but no events arrive. The terminal appears frozen.

The user sees: "Press Enter → nothing happens" because the input was cleared (step 4) and then the event loop is stuck waiting for events from a process that already sent its Result (step 9). The buffered content *was* rendered during the flush (step 5), but because the input line was just cleared and potentially overwrote it, or because it scrolled past quickly before the terminal froze, the user perceives it as "content that was supposed to be buffered isn't shown."

A similar scenario with ProcessExit instead of Result would be even worse — the process is dead, so `send_message` might error or silently fail, and the loop definitely hangs.

### Fix: resolved by the flush_event_buffer plan

The flush_event_buffer fix (making `flush_event_buffer` return `Option<SessionOutcome>`) will resolve this by:

1. When a Result is flushed, the Submit handler will see `SessionOutcome::Completed` and return early instead of trying to send the steering message to a completed session.
2. When a ProcessExit is flushed, the Submit handler will see `SessionOutcome::ProcessExited` and exit the loop.

### Verification

After the flush_event_buffer fix is implemented, test this scenario:
1. Start a session, wait for Claude to be mid-response
2. Start typing a steering message
3. Wait for Claude to finish (Result event gets buffered)
4. Press Enter to send the steering message
5. Verify: the buffered content is rendered, the session completes cleanly (or the steering message is sent as a follow-up if that's the desired behavior), and the terminal doesn't freeze.

If the flush_event_buffer fix doesn't fully resolve this, investigate the terminal rendering path — specifically whether `flush_event_buffer` output is visible to the user (cursor positioning, stdout flushing, interaction with the cleared input line).

## Questions

None — this is a dependent fix, resolved by the flush_event_buffer plan.

## Review

