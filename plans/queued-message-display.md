Issue: When there are queued messages, we should display them somehow below the messages that are streaming in. Different display for follow up and steering messages.
Status: draft

## Approach

### Problem

When a user sends a steering or follow-up message while Claude is streaming, there's no visual feedback that the message was received/queued. The user types, presses Enter/Alt+Enter, the input clears, and... nothing visible happens until the message takes effect.

### Proposed solution: Status line below streaming output

Add a "queued message" indicator that appears inline in the terminal output after the user submits a message. This avoids the complexity of scroll regions or cursor repositioning.

**For steering messages** (sent immediately via stdin):
When the user submits a steering message, render a line like:
```
⤷ steering: Actually, just count the lines instead
```
This appears at the current cursor position (after whatever Claude has streamed so far), confirming the message was sent. Claude's subsequent output continues below it.

**For follow-up messages** (buffered until Result):
When the user submits a follow-up message, render a line like:
```
⏳ queued: Can you also check the tests?
```
When the follow-up is actually sent (after Result arrives), update the display... but we can't easily go back and modify it. So just print a second confirmation:
```
⤷ follow-up: Can you also check the tests?
```

### Implementation

1. **Renderer gets new methods**:
   - `render_steering_sent(text: &str)` — prints the steering indicator inline
   - `render_followup_queued(text: &str)` — prints the queued indicator inline
   - `render_followup_sent(text: &str)` — prints when the follow-up is dispatched

2. **Session loop calls these at the right moments**:
   - In `handle_session_key_event`, after `runner.send_message()` for steering → call `renderer.render_steering_sent()`
   - In `handle_session_key_event`, when storing `pending_followup` → call `renderer.render_followup_queued()`
   - In `process_claude_event`, when dispatching a pending follow-up after Result → call `renderer.render_followup_sent()`

3. **Styling**: Use dim/muted colors to distinguish from Claude's output. The `⤷` and `⏳` prefixes visually separate these from tool calls (`[N] ▶`) and assistant text.

4. **Line management**: Before rendering the indicator, call `finish_current_block()` or at minimum ensure we're on a new line (check `text_streaming` / `tool_line_open` state). This prevents the indicator from appearing mid-word in streaming text.

### Files to modify

- `src/display/renderer.rs` — add render methods
- `src/commands/session_loop.rs` — call render methods at appropriate points
- Possibly `src/lib.rs` if `handle_inbound` needs to participate

## Questions

### Should the queued follow-up indicator persist or be cleared?

The queued follow-up message (`⏳ queued: ...`) will remain in the scrollback even after the follow-up is sent. Since we can't easily erase previously printed lines in a streaming terminal, the options are:

A. **Print both**: Show `⏳ queued` when buffered, then `⤷ follow-up` when sent. Two lines for one message, but clear timeline.
B. **Print only when sent**: Don't show anything when buffered, only show `⤷ follow-up` when actually dispatched. Simpler, but no immediate feedback.
C. **Print only when queued**: Show `⏳ queued` when buffered, nothing when sent (Claude's response to the follow-up is the implicit confirmation). Simpler, immediate feedback.

Answer:

### Should we truncate long messages in the indicator?

If the user types a very long steering/follow-up message, the indicator line could wrap and be visually noisy. Should we truncate to e.g. the first 80 characters with `...`?

Answer:

### What about multiple queued messages?

Currently `pending_followup` is `Option<String>` — only one follow-up can be queued. If the user sends a second follow-up while one is already pending, should we:

A. Replace the old one (current behavior, since it's just `Option`)
B. Queue multiple (change to `Vec<String>`)
C. Reject the second one with a message

This is tangential to the display issue but related.

Answer:

## Review

