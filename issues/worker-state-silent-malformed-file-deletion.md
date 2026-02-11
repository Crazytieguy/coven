---
priority: P2
state: new
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
