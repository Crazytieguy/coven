---
priority: P1
state: new
---

# Non-deterministic dispatch args display order

## Problem

In `src/commands/worker.rs:361-365`, the dispatch args HashMap is iterated without sorting:

```rust
let args_display = args
    .iter()
    .map(|(k, v)| format!("{k}={v}"))
    .collect::<Vec<_>>()
    .join(" ");
```

`HashMap` iteration order is non-deterministic in Rust. This `args_display` string is used in:
- `write_raw()` at line 367: `Dispatch: {agent} {args_display}` — captured in VCR snapshot tests
- `set_title()` at line 384 via `title_suffix` — visible in terminal title

Current VCR tests only exercise single-arg dispatches (`issue=...`), so the non-determinism hasn't manifested yet. Multi-arg dispatch decisions would produce flaky snapshot diffs.

## Fix

Sort the key-value pairs before joining, matching the existing pattern in `src/worker_state.rs:173-175`:

```rust
let mut args_parts: Vec<_> =
    state.args.iter().map(|(k, v)| format!("{k}={v}")).collect();
args_parts.sort();
```

Apply the same sort to the `args_display` construction in `worker.rs`.
