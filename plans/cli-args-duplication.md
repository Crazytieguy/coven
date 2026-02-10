Issue: [P2] CLI args duplication: `show_thinking`, `fork`, and `claude_args` are defined identically in three places (root `Cli`, `Ralph`, `Worker` in `cli.rs`). Extract a shared struct and use `#[command(flatten)]`.
Status: draft

## Approach

### 1. Extract `ClaudeOpts` in `cli.rs`

Create a shared struct with `#[derive(Parser)]`:

```rust
#[derive(clap::Args, Debug)]
pub struct ClaudeOpts {
    /// Stream thinking text inline in dim italic instead of collapsing.
    #[arg(long)]
    pub show_thinking: bool,

    /// Enable model-driven context forking via <fork> tags.
    #[arg(long)]
    pub fork: bool,

    /// Extra arguments to pass through to claude (after --).
    #[arg(last = true)]
    pub claude_args: Vec<String>,
}
```

Replace the three copies with `#[command(flatten)] pub claude_opts: ClaudeOpts` in root `Cli`, and `#[command(flatten)]` fields in the `Ralph` and `Worker` variants.

### 2. Update `main.rs` pattern matching

Destructure `claude_opts` from each variant. The config struct construction stays the same — just pulling from `claude_opts.show_thinking` etc. instead of bare `show_thinking`.

### 3. No changes to command config structs

`RunConfig`, `RalphConfig`, `WorkerConfig` are internal config types with different field sets (e.g. `extra_args` vs `claude_args`, `working_dir`). They don't share enough to warrant a common struct — the duplication there is incidental. Leave them as-is.

## Questions

### Does `#[arg(last = true)]` on `claude_args` work inside `#[command(flatten)]`?

Clap's `last = true` captures everything after `--`. When flattened, this should still work correctly since `flatten` inlines the args into the parent command. But this needs verification — if it doesn't work with flatten in subcommand variants, we may need to keep `claude_args` as a separate field.

Answer:

## Review

