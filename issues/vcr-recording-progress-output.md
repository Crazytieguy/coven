---
priority: P1
state: approved
---

# Add progress output to VCR recording

`cargo run --bin record-vcr` runs silently — there's no way to tell what's happening during recording. This makes it painful to use from automated workers or when recording slow multi-step orchestration tests.

## Plan

Instead of building a custom progress callback system, just let coven's normal display output through to stderr with a case-name prefix for disambiguation.

Currently the recorder passes `let mut output = Vec::new()` to each command, discarding all display output. Replace this with a `PrefixWriter` that wraps stderr and prepends `[case_name] ` to each line.

### 1. Add `PrefixWriter` in `record_vcr.rs`

A small struct wrapping `Stderr` that prepends `[{name}] ` at the start of each line:

```rust
struct PrefixWriter {
    prefix: String,
    stderr: std::io::Stderr,
    at_line_start: bool,
}
```

Implement `std::io::Write` — scan for newlines in the input buffer, inserting the prefix after each one (and at the very start).

### 2. Use `PrefixWriter` instead of `Vec::new()` in recording functions

In `record_case` and `record_multi_step`, replace:
```rust
let mut output = Vec::new();
```
with:
```rust
let mut output = PrefixWriter::new(name);  // or format!("{test_name}/{step_name}")
```

This applies in ~4 places: the run/ralph/worker/init/gc/status branches in `record_case`, and the init/worker branches in `record_multi_step`.

### 3. Files changed

- `src/bin/record_vcr.rs` only — add `PrefixWriter`, swap `Vec::new()` for `PrefixWriter::new(...)`.

### Design notes

- **No changes to `VcrContext` or any library code** — this is entirely contained in the recording binary.
- **Why stderr**: stdout could interfere with cargo output; stderr is the standard place for progress.
- **Interleaving**: cases run concurrently but are I/O-bound (waiting on Claude API), so interleaving is minimal in practice. The prefix makes it easy to follow regardless.
- **No test changes needed**: display output isn't part of VCR recordings or snapshots.
