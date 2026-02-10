Issue: [P2] Status formatting duplication: `commands/status.rs` duplicates the worker-formatting logic from `worker_state::format_status`. The two differ in minor formatting details (prefix, separator) but share the same structure. Consolidate into a single parameterized formatter.
Status: draft

## Approach

Extract the shared per-worker formatting logic into a single function in `worker_state.rs`, parameterized by a style config. Both call sites use the same structure:

1. Sort args into `k=v` pairs
2. Format each worker as: `{prefix}{branch} (PID {pid}){separator}{agent_display}` (with optional args suffix)
3. Handle idle vs active agent state

### New types and function (in `worker_state.rs`)

```rust
pub struct StatusStyle<'a> {
    pub line_prefix: &'a str,      // "  " (CLI) vs "- " (dispatch)
    pub separator: &'a str,        // " — " (CLI) vs ": " (dispatch)
    pub agent_prefix: &'a str,     // "" (CLI) vs "running " (dispatch)
}

pub fn format_workers(states: &[WorkerState], style: &StatusStyle) -> String
```

This formats a slice of `WorkerState` into a multi-line string. Each line follows the shared structure. No filtering or headers — callers handle those.

### Changes to callers

**`commands/status.rs`**: Replace inline formatting loop with:
```rust
let style = StatusStyle { line_prefix: "  ", separator: " — ", agent_prefix: "" };
let body = format_workers(&states, &style);
println!("{} active worker(s):\n\n{body}", states.len());
```

**`worker_state::format_status()`**: Replace inline formatting loop with:
```rust
let style = StatusStyle { line_prefix: "- ", separator: ": ", agent_prefix: "running " };
let body = format_workers(&others, &style);
```
Keep the existing filtering (exclude own_pid) and empty-case handling ("No other workers active.") in `format_status()`.

### Files changed

- `src/worker_state.rs` — add `StatusStyle` + `format_workers()`, refactor `format_status()` to use it
- `src/commands/status.rs` — replace inline loop with `format_workers()` call

No behavioral changes. Existing snapshot tests cover both paths.

## Questions

### Should `StatusStyle` use an enum instead of individual fields?

An enum like `StatusStyle::Cli` / `StatusStyle::Dispatch` is simpler if there are only two variants and we don't anticipate others. Individual fields are more flexible but might be over-engineering for two known use cases.

Option A: Struct with fields (proposed above) — flexible, self-documenting at call site
Option B: Enum with two variants — simpler, less room for invalid combinations

Answer:

## Review

