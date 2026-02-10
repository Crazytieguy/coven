Issue: [P0] Post-VCR-overhaul audit and cleanup
Status: approved


## Approach

Thorough cleanup pass over all files touched by the VCR system overhaul. The sections below capture known issues, but this is an audit — read each file carefully and fix anything that looks rough, not just what's listed here. Work through in order:

### 1. vcr.rs: Replace expects with proper error handling

The `Recordable` trait methods and `VcrContext::call` use `.expect()` for serde operations. Convert these to return `Result` or propagate errors with `?`. This also resolves the `missing_panics_doc` warnings since the panics go away.

Specific sites:
- `Recordable::to_recorded` / `Recordable::from_recorded` — return `Result` or use `?` internally
- `VcrContext::call` recording path — args/result serialization
- `VcrContext::call` replay path — args/result deserialization
- `TriggerEngine::new` — trigger condition parsing

### 2. ralph.rs + run.rs + record_vcr.rs: Reduce function length

The `ralph()`, `run()`, and `record_case()` functions are slightly over the 100-line limit. Extract logical sections into helper functions where there's a natural seam (e.g., the session-loop body in ralph, the resume-session block in run).

### 3. worker.rs: Context struct for too_many_arguments

Group the recurring parameter set (`extra_args`, `renderer`, `io`, `vcr`, `total_cost`) into a `WorkerContext` struct. This resolves the too_many_arguments warnings across all 7 affected functions and the P2 issue about this.

### 4. record_vcr.rs: Replace unwraps with proper error handling

`case.ralph.as_ref().unwrap()` and `case.run.as_ref().unwrap()` should use `context()` or `ok_or_else()` to produce meaningful error messages.

### 5. General audit and final pass

Read through all VCR-touched files (`vcr.rs`, `runner.rs`, `ralph.rs`, `run.rs`, `record_vcr.rs`) looking for anything else: dead code, unclear naming, missing or misleading comments, awkward control flow, unnecessary clones, etc. Fix what you find. Then run `cargo clippy`, `cargo test`, `cargo fmt` — zero warnings target.

## Questions

None — scope is mechanical cleanup, no design decisions.

## Review

