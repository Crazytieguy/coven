---
priority: P2
state: new
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
