Issue: Dead CLI flag: `--no-stream` is parsed in `cli.rs` but never used anywhere — either implement or remove
Status: draft

## Approach

Remove the `no_stream` field from the `Cli` struct in `src/cli.rs` (lines 17-19). The flag is defined but never referenced outside the struct definition — no code reads it.

This is a clean removal with no downstream impact.

## Questions

### Remove vs implement?

The flag description says "Disable partial message streaming (show only complete messages)." This would mean buffering assistant text and only showing complete messages. This seems at odds with coven's core value proposition (streaming display). Removing is simpler and avoids maintaining a feature that works against the tool's purpose.

If implementation is desired later, it can be re-added with actual wiring.

Answer:

## Review

