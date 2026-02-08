Issue: Write tool detail shows `(1 lines)` instead of `(1 line)` for single-line files — needs singular/plural handling
Status: draft

## Approach

In `src/display/renderer.rs`, line 576, the format string is hardcoded to `"({} lines)"`. Change it to use singular "line" when the count is 1.

Replace:
```rust
.map(|c| format!("({} lines)", c.lines().count()))
```

With:
```rust
.map(|c| {
    let count = c.lines().count();
    if count == 1 {
        "(1 line)".to_string()
    } else {
        format!("({count} lines)")
    }
})
```

After the fix, re-record any VCR test cases that exercise the Write tool with single-line content (if any exist), then run `cargo insta accept` if snapshots change.

## Questions

None — this is straightforward.

## Review

