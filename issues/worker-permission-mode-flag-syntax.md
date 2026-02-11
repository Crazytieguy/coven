---
priority: P0
state: approved
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

## Plan

Two changes:

1. **`src/session/runner.rs:227`** — Change `fn has_flag` to `pub(crate) fn has_flag`. The function is already `pub` within its module via `pub mod runner` in `session/mod.rs`, so `pub(crate)` makes it available crate-wide.

2. **`src/commands/worker.rs:71`** — Replace the inline check:
   ```rust
   if !config.extra_args.iter().any(|a| a == "--permission-mode") {
   ```
   with:
   ```rust
   if !crate::session::runner::has_flag(&config.extra_args, "--permission-mode") {
   ```

No new tests needed — `has_flag` already has unit tests (per issue `has-flag-tests.md`), and the worker's behavior is covered by existing VCR tests. The change is a one-line call-site swap.
