---
priority: P2
state: approved
---

# has_flag() has no unit tests

`has_flag()` in `src/session/runner.rs:227-230` was recently modified (commit `3f3498a`) to handle `--flag=value` syntax but has no unit tests:

```rust
fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter()
        .any(|a| a == flag || a.starts_with(&format!("{flag}=")))
}
```

This function controls whether default `--permission-mode` and `--max-thinking-tokens` flags are added to the Claude CLI invocation. Incorrect behavior could cause:
- Duplicate flags being passed (if false negative)
- User's flags being silently ignored by Claude (if the default overrides their value)

## Test cases to cover

- Exact match: `has_flag(&["--permission-mode".into(), "plan".into()], "--permission-mode")` → true
- Equals syntax: `has_flag(&["--permission-mode=plan".into()], "--permission-mode")` → true
- Not present: `has_flag(&["--model".into(), "opus".into()], "--permission-mode")` → false
- Substring false positive: `has_flag(&["--permission-mode-extra".into()], "--permission-mode")` → should be false, but current implementation returns true via `starts_with("--permission-mode=")` — actually this returns false since "extra" != "=...", so it's correct
- Empty args: `has_flag(&[], "--permission-mode")` → false

## Plan

Add a `#[cfg(test)] mod tests` block at the bottom of `src/session/runner.rs` with unit tests for `has_flag`. The function is already private and file-scoped, so no visibility changes needed.

### Test cases

Add these as individual `#[test]` functions:

1. **`exact_match`** — `has_flag(&["--permission-mode".into(), "plan".into()], "--permission-mode")` → `true`
2. **`equals_syntax`** — `has_flag(&["--permission-mode=plan".into()], "--permission-mode")` → `true`
3. **`not_present`** — `has_flag(&["--model".into(), "opus".into()], "--permission-mode")` → `false`
4. **`empty_args`** — `has_flag(&[], "--permission-mode")` → `false`
5. **`prefix_not_false_positive`** — `has_flag(&["--permission-mode-extra".into()], "--permission-mode")` → `false` (verifies `starts_with("--permission-mode=")` doesn't match `--permission-mode-extra`)

### Location

Append a `#[cfg(test)] mod tests { ... }` block after line 230 (end of file) in `src/session/runner.rs`. There's no existing test module in this file.
