---
priority: P0
state: approved
---

# Worker state sharing broken after agent overhaul

The recent agent overhaul broke worker state sharing. When a plan agent is working on an issue, another worker's dispatch agent sees all other workers as running dispatch agents — which isn't even possible due to the semaphore. The shared state between workers is not correctly reflecting which agent each worker is currently running.

## Plan

Two bugs in `src/commands/worker.rs` `run_agent_chain`:

### Bug 1: State update happens too late (line 341)

`vcr_update_worker_state` is called **after** the agent session completes. During the entire agent run, the worker's state file still shows whatever agent ran previously. For the first agent in a chain (dispatch), the state shows `agent: None` (idle) throughout dispatch, then gets updated to "dispatch" only after dispatch finishes — at which point the worker has already transitioned to e.g. plan.

**Fix:** Move the `vcr_update_worker_state` call from line 341 (after `run_phase_session`) to right after the semaphore acquisition at line 280, **before** the `worker_status` injection block (line 282). This way the state file reflects the current agent throughout its execution.

### Bug 2: `worker_status` arg leaks into state file (line 341)

The `worker_status` auto-injected arg (a multi-line string listing other workers) is inserted into `agent_args` at line 299. When `vcr_update_worker_state` is called at line 341, it passes `&agent_args` which now includes `worker_status`. This gets serialized into the state JSON and then displayed to other workers via `format_workers`, creating recursive nested status strings.

**Fix:** Pass a **clean copy** of the args to the state update — one that excludes `worker_status`. The simplest approach: since the state update is moving to before the `worker_status` injection (Bug 1 fix), `agent_args` won't contain `worker_status` yet, so this bug is automatically fixed by the Bug 1 fix. No additional code needed.

### Concrete change

In `run_agent_chain` (starting around line 278), change the ordering from:

```
acquire semaphore
inject worker_status into agent_args
render prompt
run session
update worker state  <-- too late, and includes worker_status in args
```

To:

```
acquire semaphore
update worker state  <-- immediate, agent_args is clean
inject worker_status into agent_args
render prompt
run session
```

Specifically:
1. Move line 341 (`vcr_update_worker_state(ctx.vcr, &wt_str, branch, Some(&agent_name), &agent_args).await?;`) to line 281 (right after the semaphore acquisition, before the `worker_status` injection block).
2. Delete the original line 341.
3. No changes needed to `worker_state.rs` or any other file.

### Validation

- Re-record the `concurrent_workers` VCR test: `cargo run --bin record-vcr concurrent_workers`
- Run `cargo test` and review snapshot diffs with `cargo insta review`
- Verify that in the recorded dispatch prompts, other workers show their actual agent (e.g. "running plan") instead of all showing "running dispatch"
- Run `cargo fmt`, `cargo clippy`
