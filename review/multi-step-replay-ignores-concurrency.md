---
priority: P2
state: review
---

# Multi-step VCR replay runs concurrent groups sequentially

In `tests/vcr_test.rs:197-210`, the `run_multi_vcr_test` function processes all steps in a flat sequential loop:

```rust
for step in &multi.steps {
    // ... runs each step one at a time
}
```

This ignores the `concurrent_group` field in the TOML config. During recording (`src/bin/record_vcr.rs:378-407`), steps in the same `concurrent_group` are launched concurrently via `spawn_local`, but during replay they execute sequentially.

This means concurrency-sensitive behavior (e.g., dispatch lock contention between two workers, race conditions in worktree state) is exercised during recording but not validated during replay. A change that breaks the concurrent path could pass tests if the sequential replay happens to succeed.

## Currently affected test

- `orchestration/concurrent_workers` — defines two worker steps in the same concurrent group, but they replay sequentially.

## Possible fix

During replay, group steps by `concurrent_group` and run grouped steps concurrently with `tokio::join!` or similar, mirroring the recording behavior.

## Plan

Mirror the recording-side grouping logic from `record_multi_case` (`src/bin/record_vcr.rs:365-409`) in the replay-side `run_multi_vcr_test` (`tests/vcr_test.rs:180-217`).

### Changes to `tests/vcr_test.rs`

**1. Add import:**
```rust
use futures::future::join_all;
```

**2. Rewrite `run_multi_vcr_test` to group by `concurrent_group`:**

Replace the flat `for step in &multi.steps` loop with a peekable iterator that mirrors the recording logic:

- Take ownership of `multi` (`case.multi.expect(...)` instead of `case.multi.as_ref().expect(...)`) so steps can be moved into async blocks.
- Iterate with `multi.steps.into_iter().peekable()`.
- For each step:
  - If `concurrent_group` is `None`: run sequentially (current behavior).
  - If `concurrent_group` is `Some(group)`: collect all consecutive steps with the same group name using `next_if`, then run them concurrently with `futures::future::join_all`.
- Each concurrent step gets its own VCR file and output buffer, same as today. After `join_all` completes, append each step's output to `combined_output` in config order (deterministic).

**Sketch of the concurrent branch:**
```rust
let group_name = step.concurrent_group.clone();
let mut group = vec![step];
while let Some(next) = steps.next_if(|s| s.concurrent_group == group_name) {
    group.push(next);
}

let futures: Vec<_> = group.into_iter().map(|step| {
    let base = &base;
    async move {
        let vcr_path = base.join(format!("{name}__{}.vcr", step.name));
        let vcr_content = std::fs::read_to_string(&vcr_path).expect("...");
        let vcr = VcrContext::replay(&vcr_content).expect("...");
        let mut output = Vec::new();
        run_multi_step(&step, &vcr, show_thinking, default_model, &mut output).await;
        let raw = String::from_utf8(output).expect("...");
        (step.name, raw)
    }
}).collect();

let results = join_all(futures).await;
for (step_name, raw) in results {
    combined_output.push_str(&format!("--- {step_name} ---\n"));
    combined_output.push_str(&strip_ansi(&raw));
    combined_output.push('\n');
}
```

### Why `join_all` over `spawn_local`

- The recording side uses `tokio::task::spawn_local` because it runs under a `LocalSet` in the recording binary's `main`. The test harness uses `#[tokio::test]` which doesn't set up a `LocalSet`.
- `futures::future::join_all` provides cooperative concurrency within the same task — futures interleave at await points, which is the same concurrency model as `spawn_local` on a single-threaded runtime.
- `futures` is already a dependency (Cargo.toml line 24).
- No need for `'static` bounds or `Clone` on `MultiStep`.

### Verification

1. `cargo test` — the `concurrent_workers` test should still pass with the existing snapshot since output order is deterministic (steps are collected in config order).
2. No re-recording needed — this change only affects replay behavior, not recorded data.
