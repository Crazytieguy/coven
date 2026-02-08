Issue: Tool line metadata (line counts, diff stats) is truncated away when file paths are long — truncation should preserve the metadata suffix and truncate only the path portion
Status: draft

## Approach

Instead of adding a structured return type, simply reorder the fields in `format_tool_detail()` so metadata comes before the path. The existing `truncate_line` will naturally truncate the path at the end, preserving the metadata.

### Changes in `src/display/renderer.rs`

In `format_tool_detail()`, change the output order for tools with metadata:

- **Edit**: currently `"{path} ({diff})"` → change to `"({diff})  {path}"`
- **Write**: currently `"{path} ({lines})"` → change to `"({lines})  {path}"`

So tool lines will look like:
```
[3] ▶ Edit  (+1 -1)  /some/long/path/that/gets/trunca...
[4] ▶ Write  (5 lines)  /some/long/path/that/gets/trunca...
```

When there's no metadata (e.g., Edit with same line count), just show the path as before.

### Unit tests

Update the existing `format_tool_detail_edit_*` and `format_tool_detail_write_*` tests to match the new field order.

### VCR snapshots

Re-record VCR fixtures and accept any snapshot diffs caused by the reordering.

## Questions

None — the approach is straightforward.

## Review

