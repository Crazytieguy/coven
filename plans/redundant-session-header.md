Issue: Redundant session header in multi-turn follow-ups: when a follow-up continues the same session, the full `Session <id> (model)` header is repeated — suppress the duplicate or show a simpler turn separator
Status: draft

## Approach

In `handle_inbound` (src/lib.rs), the `Init` event always calls `renderer.render_session_header()`. For multi-turn follow-ups in the same session, the session ID is identical, so the repeated header is noise.

**Change:** In the `Init` handler in `handle_inbound`, compare the incoming `init.session_id` against `state.session_id`. If they match (i.e., this is a follow-up turn in the same session), render a lightweight turn separator instead of the full session header. If they differ (new session, e.g., ralph mode iteration), render the full header as today.

Concretely:

1. **src/lib.rs** — In the `SystemEvent::Init` match arm, before calling `render_session_header`:
   - Check if `state.session_id.as_deref() == Some(&init.session_id)`.
   - If same session: call a new `renderer.render_turn_separator()` instead.
   - If different (or first init): call `renderer.render_session_header()` as today.
   - Then update `state.session_id` and `state.model` as before.

2. **src/display/renderer.rs** — Add `render_turn_separator`:
   - Print a dim horizontal rule or simple `---` separator, plus a blank line. Something visually lighter than the session header but enough to mark the new turn.

3. **Tests** — The existing multi-turn VCR test case (`follow_up` if it exists, or a relevant one) should show the new separator in its snapshot instead of the repeated header.

## Questions

### What should the turn separator look like?

Options:
- A dim `---` (three dashes), matching the dimmed style of the session header
- A dim `· · ·` (middle dots) for a lighter feel
- Just a blank line (minimal)

I lean toward dim `---` for visual consistency with the existing dim session header style.

Answer:

## Review
