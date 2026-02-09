Issue: [P2] Durable conflict logging: conflict files and resolution outcomes are only rendered to the terminal. Consider logging them to a file for post-mortem analysis.
Status: draft

## Approach

Write conflict resolution events to a JSONL (newline-delimited JSON) append log at `<git-common-dir>/coven/conflicts.jsonl`. Each line is a self-contained JSON object representing one event in a conflict resolution sequence.

### Event schema

```rust
struct ConflictEvent {
    timestamp: String,          // ISO 8601
    worker_pid: u32,
    branch: String,
    event: ConflictEventKind,
}

enum ConflictEventKind {
    ConflictDetected {
        files: Vec<String>,
        attempt: u32,
    },
    ResolutionComplete {
        cost: f64,
        attempt: u32,
    },
    ResolutionIncomplete {
        cost: f64,
        attempt: u32,
    },
    NudgeSent,
    NudgeSucceeded,
    NudgeFailed,
    Exhausted {
        total_attempts: u32,
    },
    Landed {
        total_cost: f64,
        total_attempts: u32,
    },
}
```

### Integration points

All changes in `src/commands/worker.rs` in `land_or_resolve()`:

1. On `WorktreeError::RebaseConflict` (line ~514): log `ConflictDetected`
2. On `ResolveOutcome::Resolved` (line ~539): log `ResolutionComplete`
3. On `ResolveOutcome::Incomplete` (line ~544): log `ResolutionIncomplete`
4. On nudge sent (line ~606): log `NudgeSent`
5. After nudge, rebase no longer in progress: log `NudgeSucceeded`
6. After nudge, rebase still in progress (line ~632): log `NudgeFailed`
7. On max attempts exhausted (line ~491): log `Exhausted`
8. On successful land after conflict resolution: log `Landed`

### Implementation

Add a small `conflict_log` module (new file `src/conflict_log.rs`) with:
- `ConflictEvent` and `ConflictEventKind` structs (serde Serialize)
- `fn log_event(git_common_dir: &Path, event: ConflictEvent) -> Result<()>` — appends one JSON line to the log file, creating it if needed

The log function opens the file in append mode each call (no persistent handle needed — conflicts are infrequent). Use `serde_json::to_string` + writeln. Errors are logged as warnings via `renderer.render_warning()` but don't fail the resolution.

### File location

`<git-common-dir>/coven/conflicts.jsonl` — same directory that already holds `workers/` state files, so no new directories needed.

### What this does NOT include

- Log rotation or cleanup (file will be small — conflicts are rare events)
- CLI commands to read/query the log (users can `cat` / `jq` it)
- Logging non-conflict events (out of scope)

## Questions

### Should warnings (session ID mismatch, clean failures) also be logged?

These are currently rendered via `render_warning()`. They're loosely related to conflicts but represent different failure modes. Including them would make the log more complete for post-mortem, but adds more event variants.

Answer:

## Review

