Issue: [P2] Test system gap: follow-up and steering messages aren't visible in snapshots because the test harness sends messages but doesn't simulate how they'd appear in the terminal.
Status: draft

## Approach

Modify `replay_stdout()` in `tests/vcr_test.rs` to render user messages from `> ` lines instead of skipping them. The first `> ` line is the initial prompt (skip it — the live app doesn't display the initial prompt either), but subsequent `> ` lines are follow-ups or steering messages that should be rendered.

### Determining message type (steering vs follow-up)

In the live app, the distinction is:
- **Steering**: sent mid-stream (while the model is producing output) — rendered as `⤷ steering: <text>`
- **Follow-up**: sent after a result — rendered as `⤷ follow-up: <text>`

During VCR replay, we can distinguish these by tracking state: if we've seen a `Result` event since the last stdin line, the next stdin message is a follow-up. Otherwise, it's steering.

### Changes

**`tests/vcr_test.rs` — `replay_stdout()`:**

1. Add a `stdin_count` tracker (to skip the first stdin line — the initial prompt).
2. Add a `seen_result` flag, set to `true` when a `Result` event is processed, reset when a stdin line is processed.
3. When encountering a `> ` line after the first one:
   - Parse the JSON to extract the `message.content` field.
   - If `seen_result` is true, call `renderer.render_followup_sent(&content)`.
   - Otherwise, call `renderer.render_steering_sent(&content)`.
   - Reset `seen_result` to false.
4. On `---` separator (ralph iteration), reset `stdin_count` to 0 so each iteration's first stdin line is treated as the initial prompt again.

**Snapshot updates:**

After implementing, re-run `cargo test` and accept updated snapshots. Expected changes:
- `steering.snap`: will gain a `⤷ steering: Actually, just count the lines in each file instead` line after tool [3] or [4]
- `multi_turn.snap`: will gain a `⤷ follow-up: How does ownership work?` line between the two sessions

Other test snapshots with only a single stdin line (initial prompt) should be unaffected.

### What we need from `handle_inbound`

We need to detect when a `Result` event is processed. Looking at `handle_inbound`, it calls `renderer.render_result()` for `InboundEvent::Result`. We can check event type before calling `handle_inbound`, or simply track it outside.

Actually, simpler: `parse_line` returns `InboundEvent` variants. We can match on the event to check if it's a `Result` before passing it to `handle_inbound`. This avoids modifying any library code.

## Questions

### Should the initial prompt also be displayed?

In the live app, the initial prompt is entered at a `> ` prompt but the text itself is shown inline. However, the test harness is more like a batch mode — the prompt is specified in the TOML and is already visible in the test case definition. Displaying it would add noise to every snapshot.

Recommendation: skip the initial prompt (first `> ` line per session/iteration).

Answer:

## Review

