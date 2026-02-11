---
priority: P2
state: review
---

# Worker state `read_all` silently deletes malformed files

## Problem

In `src/worker_state.rs:133-138`, when `read_all()` encounters a state file that can't be parsed as JSON, it silently deletes the file and continues:

```rust
let Ok(content) = fs::read_to_string(&path) else {
    continue;
};
let Ok(state) = serde_json::from_str::<WorkerState>(&content) else {
    let _ = fs::remove_file(&path);
    continue;
};
```

No warning is surfaced to the caller or logged anywhere. This makes it impossible to diagnose issues where worker state files get corrupted (e.g., from partial writes during crashes, concurrent access, or disk errors).

Similarly, at line 143-144, stale worker files (dead PIDs) are silently cleaned up — this is intentional behavior, but the malformed file case at line 137 is different: it indicates unexpected corruption rather than normal lifecycle cleanup.

## Impact

Low — worker state files are small JSON blobs and corruption is rare. But when it does happen (e.g., a worker crashes mid-write to the state file), `coven status` would silently drop that worker from the list and delete the evidence, making debugging very difficult.

## Fix

Return a warning message or add an optional callback when a malformed state file is deleted, so callers like `coven status` and the dispatch phase can surface the issue. Alternatively, `read_all` could return a struct with both the valid states and a list of cleaned-up files.

## Plan

### Approach

Change `read_all` to return a result struct that includes both the valid worker states and a list of warnings about cleaned-up malformed files. Callers print warnings to stderr via `eprintln!` (consistent with the codebase's existing pattern for non-interactive diagnostics).

### Changes

**1. `src/worker_state.rs` — new return type and updated `read_all`**

Add a struct alongside `WorkerState`:

```rust
pub struct ReadAllResult {
    pub states: Vec<WorkerState>,
    pub warnings: Vec<String>,
}
```

Update `read_all` signature: `pub fn read_all(repo_path: &Path) -> Result<ReadAllResult>`

In the malformed-file branch (current lines 136-138), before deleting the file, push a warning string like `"Deleted malformed worker state file: {filename}"` to `warnings`. Keep the deletion behavior — just make it observable.

The unreadable-file branch (lines 133-134) should also push a warning: `"Could not read worker state file: {filename}"`. (No deletion needed since the file couldn't be read.)

The dead-PID branch (lines 141-144) stays silent — that's normal lifecycle cleanup, not corruption.

**2. `src/commands/status.rs` — surface warnings**

After the VCR call, destructure the result. Print each warning to stderr with `eprintln!`. Then use `result.states` where `states` was used before.

**3. `src/commands/worker.rs` — surface warnings**

Same pattern: destructure, `eprintln!` warnings, use `result.states` for `format_status`.

**4. `src/commands/gc.rs` — surface warnings**

Same pattern: destructure, `eprintln!` warnings, use `result.states` for the live-branches set.

**5. Tests in `src/worker_state.rs`**

- Update existing `read_all_returns_live_workers` and `read_all_cleans_stale_workers` tests to destructure `ReadAllResult` and assert `warnings.is_empty()`.
- Add a new test `read_all_warns_on_malformed_file`: write a file with invalid JSON content to the workers dir, call `read_all`, assert the file is deleted, `states` doesn't contain it, and `warnings` has one entry mentioning the filename.
