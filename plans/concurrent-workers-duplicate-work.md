Issue: [P0] The concurrent workers test shows both workers implementing both issues, suggesting either the code is broken or the test is broken
Status: draft

## Root Cause

Worker state files are keyed by PID (`workers/<pid>.json`), and `format_status` filters "self" by PID. During VCR recording, both workers run in the same process via `spawn_local`, sharing the same PID. This causes:

1. **State file collision**: Both workers write to the same `<pid>.json` file, overwriting each other's state.
2. **Self-filtering removes the other worker**: `format_status(states, own_pid)` filters out entries matching `own_pid`. Since both workers share a PID, Worker B filters out Worker A's entry, seeing "No other workers active."
3. **Duplicate dispatch**: Without visibility into the other worker, the dispatch agent picks the same issues for both workers.

This is confirmed in the VCR data: both workers' dispatch prompts say "No other workers active" (visible in the `spawn` args in both VCR files). Worker B's `read_all` even shows a stale branch name ("neat-ember-33") from a previous recording, because the shared PID means state files get overwritten.

## Approach

Switch worker state identity from PID to branch name. Branch names are unique per worker even within the same process.

### Changes to `src/worker_state.rs`

1. **`state_file_path`**: Take `branch` parameter, use `{branch}.json` instead of `{pid}.json`.

2. **`register`**: Already takes `branch`. Use it for file naming via updated `state_file_path`. PID still stored in `WorkerState.pid` (via `std::process::id()`) for liveness checking in `read_all`.

3. **`update`**: Already takes `branch`. Use it for file naming.

4. **`deregister`**: Add `branch: &str` parameter. Use it for file naming.

5. **`format_status`**: Change signature from `(states, own_pid: u32)` to `(states, own_branch: &str)`. Filter by `s.branch != own_branch` instead of `s.pid != own_pid`.

### Changes to `src/commands/worker.rs`

1. **Remove `process_id` VCR call**: No longer needed — PID was only used for `format_status`, which now uses branch.

2. **Update `format_status` call**: Pass `branch` instead of `own_pid`.

3. **Update `deregister` VCR call**: Pass branch alongside repo path (change args from `String` to `(String, String)`).

### Changes to `src/worker_state.rs` unit tests

- `format_status_no_others`: Filter by branch name instead of PID.
- `format_status_with_others`: Filter by branch name instead of PID.

### VCR re-recording

Re-record `concurrent_workers` and `worker_basic`. Accept new snapshots. The concurrent_workers snapshot should now show each worker picking a different issue.

## Questions

### Should the production `read_all` liveness check remain PID-based?

Currently `read_all` checks `is_pid_alive(state.pid)` to clean up stale entries. With branch-named files, the PID in the state is still the process PID, and liveness checking still works. The only edge case: if a worker crashes and restarts with a new PID but the same branch name, the old file would be overwritten (correct behavior). If a worker crashes and a different branch is assigned, the old file stays until `read_all` detects the stale PID.

I believe this is fine as-is. No change needed to liveness checking.

Answer:

## Review

