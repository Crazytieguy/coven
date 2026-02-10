Issue: [P0] Consider deduplicating the vcr recording infrastructure: don't have separate code paths for the different coven commands. Plan should either include an implementation plan or clear reasoning why duplication is better.
Status: draft

## Approach

Add a `TestCase::run_command()` method that encapsulates the branching logic, so both `record_vcr.rs` and `vcr_test.rs` call one function instead of duplicating the same if/else chain.

No new config structs are needed — `run()` keeps its positional args, and `run_command()` constructs the existing `RalphConfig`/`WorkerConfig` structs directly.

### Why not reuse the clap structs?

The clap CLI types can't be reused for test dispatch because:
1. `Cli` carries `command: Option<Command>` which is irrelevant to any single command function
2. Subcommand args (`Ralph`, `Worker`) are enum variants, not standalone structs — can't be passed to functions
3. None of the clap types have `working_dir` (test-only; always `None` from CLI, `Some(tmp_dir)` during recording)
4. `Worker.worktree_base` is `Option<PathBuf>` in clap (defaulted in `main.rs`) but `PathBuf` in `WorkerConfig`

The existing `RalphConfig` and `WorkerConfig` already bridge this gap for their commands. `run()` currently takes positional args — this is fine and doesn't need a config struct just for symmetry.

### Step 1: Add `TestCase::run_command()`

Add a method on `TestCase` (in `vcr.rs`) that builds the appropriate config and calls the right command function. Signature:

```rust
impl TestCase {
    pub async fn run_command<W: Write>(
        &self,
        io: &mut Io,
        vcr: &VcrContext,
        writer: W,
        working_dir: Option<PathBuf>,
    ) -> Result<Vec<StoredMessage>> { ... }
}
```

This method:
1. Determines which command to run (`is_worker()` / `is_ralph()` / default)
2. Extracts the command-specific test config
3. Injects `--model` default if not present
4. For worker: creates worktree_base dir (as sibling of `working_dir` or a temp path) and cleans up after
5. Calls the command function (`run()` with positional args, `ralph()` with `RalphConfig`, `worker()` with `WorkerConfig`)
6. Returns `Vec<StoredMessage>` (empty vec for worker, which returns `()`)

### Step 2: Simplify callers

**`record_vcr.rs`** (lines 151-212) becomes:
```rust
case.run_command(&mut io, &vcr, &mut output, Some(tmp_dir.clone())).await?;
```

**`vcr_test.rs`** (lines 54-117) becomes:
```rust
let messages = case.run_command(&mut io, &vcr, &mut output, None).await
    .expect("Command failed during VCR replay");
```

### Auto-exit logic (no change needed)

The auto-exit decision (lines 138-139 of `record_vcr.rs`) is recording-only. It stays in `record_vcr.rs` since it configures the `TriggerController` before `run_command` is called.

## Questions

### Should `run_command` live on `TestCase` or be a free function?

`TestCase` already has `is_worker()`, `is_ralph()`, and holds the config data, so a method seems natural. The alternative is a free function `run_test_command(case, io, vcr, writer, working_dir)` — functionally equivalent but less discoverable.

I lean toward the method since it keeps the dispatch logic next to the config types.

Answer:

## Review

