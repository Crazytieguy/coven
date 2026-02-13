# coven

An oven for claude - a minimal streaming display and workflow runner for Claude Code's `-p` mode.

## Project Overview

See @README.md for user-facing documentation.

## Conventions

- Use `thiserror` for error types, `anyhow` for application errors
- Prefer small, focused modules
- Add dependencies with `cargo add` to get latest versions
- Run `cargo fmt`, `cargo clippy`, and `cargo test` before committing
- Never run `cargo doc --open`
- Dependency docs available in `target/doc-md/`, index: @target/doc-md/index.md
- Regenerate docs after adding a dependency with `cargo doc-md`
- Never write `.vcr` files directly — they must only be created or modified by `cargo run --bin record-vcr`. Prefer re-recording specific cases: `cargo run --bin record-vcr simple_qa` re-records one, `cargo run --bin record-vcr simple_qa follow_up` re-records a few. Re-recording all fixtures (`cargo run --bin record-vcr`) is expensive — only do it when changes affect many tests (e.g. prompt or system changes). After re-recording, run `cargo test` to see snapshot diffs, iterate as needed, then accept with `cargo insta accept`. Always run VCR recordings with a 1 minute timeout (using the Bash tool parameter) — they can hang indefinitely.
- Always prefer properly VCR-recording I/O operations over working around them. Every external I/O call (filesystem, process info, network, etc.) should go through `vcr.call()` so it's recorded during recording and replayed deterministically during tests. Never use `vcr.is_live()`/`vcr.is_replay()` guards to skip I/O — instead, wrap the I/O in a VCR call.
- VCR tests aren't just for CLI functionality — orchestration tests are also evals that check how well models pilot the system given our prompts and agents. Improving prompts can be validated by re-recording and checking snapshot diffs.
- Never add `#[allow(...)]` attributes or allow lint rules in `Cargo.toml` without verifying with the user
- Never make security-relevant decisions without confirmation. This includes permission modes, authentication, access control, and anything that affects the trust boundary of the system. Always apply least-privilege: when granting permissions to spawned agents (e.g. in test fixtures), allow only the specific commands needed, never broad wildcards like `Bash(*)`.
- Update README.md when adding/removing commands, changing flags, or altering user-facing behavior. Keep it under 100 lines.

## Publishing

Don't publish or release without asking.

1. Bump version in `Cargo.toml` (patch version unless told otherwise)
2. Update README.md if needed
3. `cargo publish --allow-dirty`
4. `git tag -a vX.Y.Z -m "Release vX.Y.Z" && git push origin vX.Y.Z`
5. GitHub Actions builds binaries and updates Homebrew tap automatically

## Bash Operations

Complex bash syntax is hard for Claude Code to permission correctly. Keep commands simple.

Simple operations are fine: `|`, `||`, `&&`, `>` redirects.

For bulk operations on multiple files, use xargs:
- Plain: `ls *.md | xargs wc -l`
- With placeholder: `ls *.md | xargs -I{} head -1 {}`

Avoid string interpolation (`$()`, backticks, `${}`), heredocs, loops, and advanced xargs flags (`-P`, `-L`, `-n`) - these require scripts or simpler alternatives.

**Patterns:**
- File creation: Write tool, not `cat << 'EOF' > file`
- Env vars: `export VAR=val && command`, not `VAR=val command` or `env VAR=val command`
- Bulk operations: `ls *.md | xargs wc -l`, not `for f in *.md; do cmd "$f"; done`
- Parallel/batched xargs: use scripts, not `xargs -P4` or `xargs -L1`
- Per-item shell logic: use scripts, not `xargs sh -c '...'`
- Issue summary: `head -7 issues/*.md review/*.md 2>/dev/null || true`

If a command that should be allowed is denied, or if project structure changes significantly, ask about running `/mats:permissions` to update settings.

## Running Commands

Run git commands plainly from the project root. Don't use `git -C`.

Run builds/tests via `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt`.

**Ad-hoc scripts:** Only `/tmp/claude-execution-allowed/coven/` is approved for ad-hoc scripts. Write bash scripts there and run with `bash /tmp/claude-execution-allowed/coven/<script-name>`.

When you create a new reusable script, offer to add a permission for it. Example: "I created scripts/analyze.sh. Want me to add `Bash(bash scripts/analyze.sh *)` to your permissions?"

For string interpolation, heredocs, loops, or advanced xargs flags, write a script in `/tmp/claude-execution-allowed/coven/` instead.
