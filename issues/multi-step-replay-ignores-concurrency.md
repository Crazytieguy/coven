---
priority: P2
state: new
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
