Issue: We should use the terminal width to truncate rendering more accurately
Status: rejected

## Approach

Currently, tool detail lines (bash commands, error messages, tool parameters) are truncated to a hardcoded 60 characters via `first_line_truncated()`. This should adapt to the actual terminal width.

### Changes

1. **Add `term_width` to `RendererConfig`** (`src/display/renderer.rs`):
   - Add `pub term_width: u16` field
   - Query `crossterm::terminal::size()` at renderer creation time (in `commands/run.rs`)
   - Default to 80 if the query fails

2. **Pass available width to `first_line_truncated()`**:
   - Change `first_line_truncated(s, 60)` calls to compute available width from `term_width` minus the line prefix length (e.g. `[N] > toolname  ` is ~20-30 chars depending on tool name and index)
   - The 4 call sites in `renderer.rs` (lines 285, 346, 557, 569) all need updating

User note: the fact we have 4 call sites that need updating tells me we should probably find a more DRY solution

3. **Compute available width per context**:
   - In `format_tool_detail()`, the caller (`render_tool_use` at line 457) formats `[{index}] ▶ {name}  {detail}`. The prefix length is knowable: `[` + index digits + `] ▶ ` + name length + `  ` = ~`5 + index_width + name.len()` characters.
   - Pass the remaining width to `format_tool_detail()` so each truncation adapts.
   - For error messages (line 285) and subagent results (line 346), the prefix is different — compute accordingly.

User note: make sure this is exact. Ideally the code should be such that this is computed from the actual content so that it stays in sync

4. **Handle terminal resize** (optional, low priority):
   - Could listen for `Event::Resize` in the input handler and update width, but this adds complexity. A simpler approach: re-query `terminal::size()` each time we render a tool line. The call is cheap (single syscall). This avoids needing mutable shared state for width.

User note: not optional, let's do it

### Recommended approach for width query

Rather than storing width in config (which becomes stale on resize), have `first_line_truncated` accept a `max` parameter computed at each call site from a fresh `terminal::size()` query. This is simple and handles resize automatically. Wrap the query in a small helper:

```rust
fn term_width() -> usize {
    crossterm::terminal::size().map(|(w, _)| w as usize).unwrap_or(80)
}
```

Then at each truncation call site, compute `term_width() - prefix_len` and pass that as `max`.

This means we don't need to change `RendererConfig` at all — just add the helper and update the 4 call sites.

## Questions

### Should we also handle wide Unicode characters?

The current `first_line_truncated` uses `.len()` which counts bytes, not display width. For accurate truncation we'd need `unicode-width` crate. This is a separate concern — file paths and bash commands are almost always ASCII.

Answer: Yes

## Review
