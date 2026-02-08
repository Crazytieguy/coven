Issue: Dead code in Edit diff stats: the `(true, true)` branch in `format_tool_detail` for Edit (`+N -M` format) is unreachable — `saturating_sub` means if added > 0 then removed = 0 and vice versa
Status: draft

## Approach

In `src/display/renderer.rs` lines 559-567, the Edit branch computes:

```rust
let added = new_lines.saturating_sub(old_lines);
let removed = old_lines.saturating_sub(new_lines);
```

Because `saturating_sub` clamps to 0, it's impossible for both `added > 0` and `removed > 0` to be true simultaneously. The `(true, true)` match arm (line 563) is dead code.

Replace the four-arm match with a simpler conditional:

```rust
let diff = if added > 0 {
    format!("+{added}")
} else {
    format!("-{removed}")
};
```

The outer `if added > 0 || removed > 0` guard already ensures we only reach this code when at least one is positive, so no `(false, false)` case is needed.

The existing test `format_tool_detail_edit_net_additions` (line 835) already validates that a 3-line old / 5-line new produces `(+2)` not `(+2 -0)`, confirming the intended behavior. No new tests needed.

## Questions

None — straightforward dead code removal.

## Review

