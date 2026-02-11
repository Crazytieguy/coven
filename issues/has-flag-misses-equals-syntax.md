---
priority: P2
state: new
---

# `has_flag` doesn't detect `--flag=value` syntax

`src/session/runner.rs:227-228` — `has_flag()` checks for exact string equality:

```rust
fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}
```

This only matches `["--permission-mode", "plan"]` (two separate args), not `["--permission-mode=plan"]` (single arg with `=`).

**Impact:** If a user passes `-- --permission-mode=plan`, coven adds a second `--permission-mode acceptEdits` (lines 127-129), resulting in two conflicting `--permission-mode` flags on the Claude CLI command. Same issue for `--max-thinking-tokens=N`.

**Fix:** Also check for the `starts_with` prefix:

```rust
fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag || a.starts_with(&format!("{flag}=")))
}
```
