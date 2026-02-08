Issue: Tool line metadata (line counts, diff stats) is truncated away when file paths are long — truncation should preserve the metadata suffix and truncate only the path portion
Status: draft

## Approach

The current flow builds a flat string like `"/some/long/path (+1 -1)"` in `format_tool_detail()`, then truncates the entire composed line `"[N] ▶ Edit  /some/long/path (+1 -1)"` left-to-right, losing the metadata suffix when paths are long.

**Change `format_tool_detail()` to return a structured type** that separates the truncatable body from the preserved suffix:

```rust
struct ToolDetail {
    body: String,           // truncatable (path, command, etc.)
    suffix: Option<String>, // preserved (e.g. "(+1 -1)", "(5 lines)")
}
```

Tools that produce metadata suffixes (Edit, Write) return them in `suffix`. All others return `suffix: None`.

**Change the two call sites** (lines 479 and 314 in `renderer.rs`) to:

1. Build the prefix: `"[N] ▶ ToolName  "` (or `"  [N] ▶ ToolName  "` for subagents)
2. Compute display width of prefix + suffix (with a space separator if suffix is present)
3. Truncate `detail.body` to `term_width() - prefix_width - suffix_width` using `truncate_to_width()`
4. Compose the final line: `prefix + truncated_body + " " + suffix`

This way the metadata suffix is always visible, and only the path portion gets `...` truncation.

**Edge case:** If the prefix + suffix alone exceed terminal width, fall back to truncating the entire composed string (current behavior) — the metadata can't be preserved if there's no room at all.

**Files changed:**
- `src/display/renderer.rs`: add `ToolDetail` struct, update `format_tool_detail()` return type, update both call sites, add tests

**Testing:**
- Add unit tests for the new truncation behavior (path with suffix at various widths)
- Re-record VCR fixtures and check snapshots for any visual changes (the `ralph_break` case is the known example where metadata was lost)

## Questions

### Should `ToolDetail` use `Display` trait or explicit composition?

We could impl `Display` for `ToolDetail` for the no-truncation case (e.g. stored messages), and have a separate `ToolDetail::truncated(width)` method for rendering. Alternatively, keep it simple and just destructure at the call sites.

Answer:

## Review

