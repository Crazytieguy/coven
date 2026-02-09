Issue: If there's a queued follow up message, don't print the Done line and session separator, just the follow up. And when a follow up is submitted, also don't display the separator (---)
Status: draft

## Approach

Two related changes to make follow-ups feel like continuous conversation rather than session restarts:

### 1. Suppress Done line when there's a queued follow-up

Currently in `session_loop.rs:process_claude_event`, `handle_inbound()` is called unconditionally on line 172, which renders the Done line for Result events. Then on line 180 we check for pending follow-ups.

**Fix:** Check for pending follow-ups *before* calling `handle_inbound` when the event is a Result. If there are pending follow-ups, call `handle_inbound` with a signal to skip rendering the result line — or more cleanly, split the Result handling:

- Extract the state updates from `handle_inbound`'s Result arm (cost, turns, duration, status) into a separate path in `process_claude_event`
- When there are pending follow-ups: apply state updates but skip `render_result()`
- When there are no pending follow-ups: call through to `render_result()` as normal

Concretely, add an optional `suppress_result: bool` parameter or a separate method. The cleanest approach: make `handle_inbound` return a flag or accept a context that says whether to render the result, OR move the result rendering out of `handle_inbound` into `process_claude_event` where we already have the follow-up context.

**Chosen approach:** Add a `has_pending_followups: bool` parameter to `handle_inbound`. When true and the event is a Result, update state but skip `render_result()`. This keeps the logic centralized in `handle_inbound` while giving the caller control.

Changes:
- `src/lib.rs`: Add `has_pending_followups: bool` param to `handle_inbound`, conditionally skip `render_result()` when true
- `src/commands/session_loop.rs`: Pass `!locals.pending_followups.is_empty()` when calling `handle_inbound`

### 2. Suppress --- separator after follow-up

When a follow-up is sent (queued or user-typed), Claude resumes the same session. The Init event matches the session ID, so `handle_inbound` renders `---`.

**Fix:** Add a `suppress_next_separator: bool` field to `SessionState`. Set it to `true` when sending a follow-up. In `handle_inbound`'s Init handler, check and clear this flag — if set, skip `render_turn_separator()`.

Changes:
- `src/session/state.rs`: Add `suppress_next_separator: bool` field (default false)
- `src/lib.rs`: In Init handler, check `state.suppress_next_separator` before rendering separator; clear the flag either way
- `src/commands/session_loop.rs`: Set `state.suppress_next_separator = true` before sending queued follow-ups (line ~188) and before sending user-typed follow-ups (in `wait_for_followup`)

### 3. Tests

Re-record VCR fixtures that involve follow-ups (if any exist), review snapshots to verify the Done line and separator are gone. If no follow-up fixtures exist, consider adding one — but this may be out of scope for this issue.

## Questions

### Should the cost/duration/turns stats from the suppressed Done line be shown elsewhere?

When we suppress the Done line for a queued follow-up, the user loses visibility into intermediate cost/turns. Options:
- **Drop them silently** — the final Done line (when no more follow-ups are queued) will show cumulative totals anyway since Claude tracks these per-session
- **Show a compact inline stat** — e.g., after the `⤷ follow-up:` line, show `($0.05 · 3 turns)` in dim text
- **Don't worry about it** — users who queue follow-ups are in flow and don't care about intermediate stats

Answer:

## Review
