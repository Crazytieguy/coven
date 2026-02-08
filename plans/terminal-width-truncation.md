Issue: We should use the terminal width to truncate rendering more accurately
Status: draft

## Approach

Currently, tool detail lines and error messages are truncated to a hardcoded 60 characters via `first_line_truncated()`. This should adapt to the actual terminal width, use unicode display widths, and handle resizes.

### Key design: truncate whole lines, not fragments

The current approach truncates detail text inside `format_tool_detail()` and at each error-rendering call site — 4 separate places that each need to know the available width. Instead, we move truncation to the point where the complete line is assembled, so the prefix width is computed from actual content rather than estimated.

### Changes

1. **Add `unicode-width` crate** (`cargo add unicode-width`):
   - Use `UnicodeWidthStr::width()` for display-width-aware truncation instead of byte `.len()`.

2. **Replace `first_line_truncated()` with two functions**:
   - `first_line(s: &str) -> &str` — extracts the first line, no truncation. Used inside `format_tool_detail()` for Bash commands and unknown tools.
   - `truncate_to_width(s: &str, max_width: usize) -> String` — truncates a string to `max_width` display columns using `unicode-width`, appending `...` if truncated. Operates on chars, accumulating display width until the limit.

3. **Add `term_width()` helper**:
   ```rust
   fn term_width() -> usize {
       crossterm::terminal::size().map(|(w, _)| w as usize).unwrap_or(80)
   }
   ```
   Called fresh each time to handle terminal resizes automatically — no stored state needed.

4. **Add `truncate_line()` method on Renderer** — the single place where line truncation happens:
   ```rust
   fn truncate_line(&self, line: &str) -> String {
       truncate_to_width(line, term_width())
   }
   ```
   This is called right before `queue!(Print(...))` for any line that could exceed terminal width: tool call lines, subagent tool call lines, and error detail lines.

5. **Update `format_tool_detail()`** — remove truncation from inside this function:
   - Change internal `first_line_truncated(s, 60)` calls to `first_line(s).to_string()`.
   - The function now returns untruncated detail text. Truncation happens in the caller via `truncate_line()`.

6. **Update the 3 rendering sites** to use `truncate_line()`:
   - `finish_current_block()` (tool use case, line ~462): after assembling `let label = format!("[{n}] ▶ {name}  {detail}")`, pass through `self.truncate_line(&label)`.
   - `render_subagent_tool_call()` (line ~305): same pattern with the `  [{n}] ▶ {name}  {detail}` line.
   - Error rendering in `render_tool_result()` and `render_subagent_tool_result()`: assemble the full error line (`{indent}✗ {brief}`) and pass through `self.truncate_line()`.

   The error rendering paths (lines ~282-288 and ~342-350) currently do truncation + output in separate queue calls. Refactor each into: assemble full line string → truncate → single Print.

### Why this is DRY

- `format_tool_detail()` no longer truncates — it just formats.
- All width-aware truncation flows through `truncate_line()` → `truncate_to_width()`.
- Prefix width is computed from the actual assembled string, not estimated — stays in sync automatically.
- `term_width()` is called fresh each time, handling resizes with no stored state.

### Files to change

- `Cargo.toml` — add `unicode-width`
- `src/display/renderer.rs` — all changes are in this one file:
  - Add `term_width()`, `first_line()`, `truncate_to_width()` free functions
  - Add `truncate_line()` method on `Renderer`
  - Update `format_tool_detail()` to not truncate
  - Update the 3 rendering sites to use `truncate_line()`
  - Remove `first_line_truncated()`

## Questions

None — all previous questions answered.

## Review

