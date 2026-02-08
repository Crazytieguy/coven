Issue: In ralph mode, the raw `<break>reason</break>` XML tag appears in Claude's streamed text output. The break reason is then also displayed cleanly as "Loop complete: reason" after the stats line. The raw XML should be stripped from the rendered text to avoid duplication and ugly output.
Status: rejected

## Approach

The problem: text deltas are streamed to the terminal in real time via `renderer.stream_text()`. When Claude emits `<break>reason</break>`, those characters appear on screen before the Result event is processed. The "Loop complete: reason" line then shows the same info cleanly, creating duplication.

### Streaming filter in session_loop

Add a break-tag filter to the text streaming path, active only in ralph mode. The filter watches for the opening `<` that starts the break tag and suppresses output from that point onward.

#### How it works

1. Add an optional `break_tag: Option<String>` field to `SessionState` (set when running in ralph mode, `None` otherwise).
2. Add a `break_tag_buffer: String` field to `SessionState` for accumulating potential tag characters.
3. In `process_claude_event`, before calling `renderer.stream_text()` for text deltas, pass the text through a filter function.

The filter logic:

- **Normal state**: Scan each text delta for `<`. If no `<` found, pass through entirely. If `<` found, pass through everything before it, then start buffering from `<` onward.
- **Buffering state**: Accumulate characters. At each step, check if the buffer still matches a prefix of `<{tag}>...</{tag}>`. If yes, keep buffering (don't display). If the buffer is confirmed NOT to be the break tag (e.g., `<br` when tag is `break` — wait, that IS a prefix), then flush the buffer to display and return to normal state. If the full closing tag is found, discard the entire match (suppress it).
- **End of response**: If buffering is active when the response ends (e.g., partial `<break>` without closing), flush the buffer since it wasn't a complete tag.

This is the correct but complex approach. There's a simpler alternative:

### Simpler alternative: post-hoc cleanup

Since the break tag typically appears at the very end of Claude's response, and the "Loop complete" line is printed after the stats, the visual impact is limited. A simpler approach:

1. After the session completes in ralph mode and a break tag is detected, use ANSI escape sequences to erase the lines containing the raw tag.
2. This avoids modifying the streaming path entirely.

However, this is fragile — it requires knowing how many terminal lines the break tag occupied, which depends on terminal width and tag content length.

### Recommended: streaming filter (simplified)

The full prefix-matching filter is overkill. In practice, `<break>` is unlikely to appear in normal Claude output, and if it does, briefly suppressing a few characters of output is harmless. A simplified version:

1. Add `break_tag_filter: Option<BreakTagFilter>` to `SessionState`.
2. `BreakTagFilter` holds: `tag: String`, `state: FilterState` (enum: `Normal`, `Buffering(String)`).
3. A method `filter(&mut self, text: &str) -> String` that:
   - In `Normal`: finds `<`, passes through text before it, starts `Buffering` with `<`.
   - In `Buffering`: appends to buffer. If buffer contains the full `<tag>...</tag>`, return empty (suppress). If buffer is long enough that it clearly can't match (doesn't start with `<{tag}`), flush buffer + new text to output, return to `Normal`.
4. A method `flush(&mut self) -> String` called at end of response to emit any remaining buffer.

#### Files to modify

- **`src/commands/session_loop.rs`**: Add `break_tag_filter` to `SessionState`. Apply filter before `stream_text()`. Flush on Result/end.
- **`src/commands/ralph.rs`**: Pass `break_tag` to `SessionState` when constructing it.
- **New struct `BreakTagFilter`**: Could live in `src/session/` or inline in session_loop. Small enough for a dedicated struct with unit tests.

#### Edge cases

- **Tag split across many deltas**: The buffer accumulates across deltas — handled naturally.
- **Incomplete tag at end of response**: `flush()` emits the buffer so nothing is silently lost.
- **Multiple `<` characters in normal text**: Each `<` triggers buffering, but non-matching sequences are flushed quickly (as soon as the buffer diverges from `<{tag}...`).
- **Break tag in the middle of text** (rare): Works correctly — text before is displayed, tag is suppressed, text after is displayed.

## Questions

### Where should BreakTagFilter live?

Options:

- `src/session/filter.rs` — new module, clean separation
- `src/commands/session_loop.rs` — inline, since it's only used there
- `src/session/runner.rs` — alongside `scan_break_tag`, since they're related

Answer:

## Review

Honestly I don't mind the current behavior, the break tag is a low cost to pay for transparency in the display. Document somewhere that this behavior is desired so the issue isn't raised again (probably in the code, maybe in the test file, your choice)
