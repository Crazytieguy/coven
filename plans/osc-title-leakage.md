Issue: VCR test snapshots show title escape sequence leakage: OSC title sequences partially render as visible text
Status: draft

## Approach

The `strip_ansi()` function in `tests/vcr_test.rs:10-26` only handles CSI sequences (`ESC [ ... <alpha>`) but not OSC sequences (`ESC ] ... BEL` or `ESC ] ... ESC \`). When it encounters an OSC sequence like `\x1b]2;coven: clever-ember-51\x07`, it skips until the first alphabetic character (`o` in `oven:`), discarding `\x1b]2;o` and leaving `ven: clever-ember-51\x07` as visible text in the snapshot.

**Fix**: Rewrite `strip_ansi()` to distinguish sequence types after `ESC`:

1. `ESC [` (CSI) — skip until ASCII alphabetic (existing behavior, correct)
2. `ESC ]` (OSC) — skip until `\x07` (BEL) or `ESC \` (ST)
3. Any other `ESC X` — skip just the one character after ESC (simple two-byte sequences like `ESC =`)

The function stays in the same file, same purpose — just handles more escape sequence types.

After fixing, re-record worker VCR fixtures (`cargo run --bin record-vcr worker_basic` and any other worker tests) and regenerate snapshots. The snapshots should no longer contain leaked title text like `oven: clever-ember-51` or truncated text like `emoving worktree`.

### Files changed

- `tests/vcr_test.rs` — rewrite `strip_ansi()` to handle OSC sequences

### Verification

- `cargo test` — worker snapshots should be cleaner (update with `cargo insta accept`)
- Grep snapshots for `oven:` and `\x07` to confirm no residual leakage
