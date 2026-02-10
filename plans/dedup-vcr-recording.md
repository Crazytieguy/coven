Issue: [P0] Consider deduplicating the vcr recording infrastructure: don't have separate code paths for the different coven commands. Plan should either include an implementation plan or clear reasoning why duplication is better.
Status: draft

## Approach

Add a `TestCase::run_command()` method that encapsulates the branching logic, so both `record_vcr.rs` and `vcr_test.rs` call one function instead of duplicating the same if/else chain.

### Step 1: Give `run()` a config struct

`ralph()` and `worker()` take config structs, but `run()` takes positional args:

```rust
pub async fn run(prompt: Option<String>, extra_args: Vec<String>,
                 show_thinking: bool, working_dir: Option<PathBuf>,
                 io: &mut Io, vcr: &VcrContext, writer: W)
```

Introduce `RunConfig` (the runtime config, distinct from the existing test-TOML `vcr::RunConfig`):

```rust
pub struct RunConfig {
    pub prompt: Option<String>,
    pub show_thinking: bool,
    pub extra_args: Vec<String>,
    pub working_dir: Option<PathBuf>,
}
```

Update `run()` to take `RunConfig` and update `main.rs` callsite.

### Step 2: Add `TestCase::run_command()`

Add a method on `TestCase` (in `vcr.rs`) that builds the appropriate runtime config and calls the right command function. Signature:

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
4. Builds the runtime config struct
5. For worker: creates worktree_base dir (as sibling of `working_dir`) and cleans up after
6. Calls the command function
7. Returns `Vec<StoredMessage>` (empty vec for worker, which returns `()`)

### Step 3: Simplify callers

**`record_vcr.rs`** (lines 151-212) becomes:
```rust
case.run_command(&mut io, &vcr, &mut output, Some(tmp_dir.clone())).await?;
```

**`vcr_test.rs`** (lines 54-117) becomes:
```rust
let messages = case.run_command(&mut io, &vcr, &mut output, None).await
    .expect("Command failed during VCR replay");
```

### Step 4: Move auto-exit logic into `run_command` or alongside it

The auto-exit decision (lines 138-139 of record_vcr.rs) is only relevant during recording, not replay. It stays in `record_vcr.rs` since it configures the `TriggerController` before `run_command` is called. No change needed here.

## Questions

### Should `run_command` live on `TestCase` or be a free function?

`TestCase` already has `is_worker()`, `is_ralph()`, and holds the config data, so a method seems natural. The alternative is a free function `run_test_command(case, io, vcr, writer, working_dir)` — functionally equivalent but less discoverable.

I lean toward the method since it keeps the dispatch logic next to the config types.

Answer:

### Naming collision: `vcr::RunConfig` vs `commands::run::RunConfig`

There's already a `RunConfig` in `vcr.rs` (the TOML deserialization struct). The new runtime config in `commands::run.rs` would also be `RunConfig`. These are in different modules so there's no Rust conflict, but it could be confusing. Options:

1. Keep both as `RunConfig` — module paths disambiguate (`vcr::RunConfig` vs `commands::run::RunConfig`)
2. Rename the TOML one to `RunTestConfig` to match `WorkerTestConfig`
3. Rename the runtime one to `RunParams` or similar

I lean toward option 2 since `WorkerTestConfig` already sets this convention (the test TOML struct for worker is `WorkerTestConfig`, not `WorkerConfig`).

Answer:

## Review

