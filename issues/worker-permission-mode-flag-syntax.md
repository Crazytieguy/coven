---
priority: P0
state: new
---

# Worker permission-mode check doesn't handle `--flag=value` syntax

In `src/commands/worker.rs:71`, the worker checks whether the user passed a permission mode:

```rust
if !config.extra_args.iter().any(|a| a == "--permission-mode") {
    config.extra_args.extend(["--permission-mode".to_string(), "acceptEdits".to_string()]);
}
```

This only matches `--permission-mode` as a standalone arg. If the user passes `--permission-mode=plan` (equals syntax), the check fails and the worker appends a duplicate `--permission-mode acceptEdits`, overriding the user's explicit choice.

The `has_flag` function in `src/session/runner.rs:227-230` already handles both syntaxes correctly:

```rust
fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag || a.starts_with(&format!("{flag}=")))
}
```

## Fix

Make `has_flag` `pub(crate)` and use it in `worker.rs:71` instead of the manual comparison.
