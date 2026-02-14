# Audit: Architectural Issues

## Done

### Fix 1: ralph-specific methods on SessionRunner
`scan_break_tag` and `ralph_system_prompt` were `pub` methods on `SessionRunner` but only used by `ralph.rs`. Moved them to module-level functions in ralph.rs. SessionRunner now only handles subprocess lifecycle.

### Fix 2: handle_inbound in lib.rs
`handle_inbound` was the only function in `lib.rs` besides module re-exports, making it look like the library's primary entry point. It was only called from `session_loop.rs`. Moved it there and made it `fn` (private). `lib.rs` is now a clean module re-export file.

## Observations (no action taken)

### Architecture is generally well-structured
The codebase has clear layers: CLI -> commands -> core subsystems. Module boundaries are mostly clean and responsibilities well-separated. VCR testing is excellent. The session_loop provides good shared infrastructure for all session types.

### worker.rs is large (~919 lines) but well-organized
Functions are focused: `worker()` for setup/teardown, `worker_loop()` for outer loop, `run_agent_chain()` for agent chaining, `run_phase_session()` for single sessions, etc. `PhaseContext` bundles mutable state nicely. No clear split point that would improve readability. The VCR wrapper functions are boilerplate but necessary.

### No remaining architectural issues found
The rest of the codebase has clean module boundaries. Specific observations:
- `protocol/` (parse, emit, types) is well-isolated
- `display/` (renderer, input, theme, tool_format) is cohesive
- `session/` (runner, state) cleanly separates process management from state tracking
- Orchestration modules (agents, transition, worktree, worker_state, semaphore) have focused responsibilities
- `fork.rs` is self-contained with clear interfaces to session_loop

## Status
Review session needed to verify the full diff before landing.
