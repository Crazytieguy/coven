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
- Changelog follows [Keep a Changelog](https://keepachangelog.com/) format
- Never write `.vcr` files directly — they must only be created or modified by `cargo run --bin record_vcr`. Re-recording is cheap enough: `cargo run --bin record_vcr` re-records all fixtures, `cargo run --bin record_vcr simple_qa` re-records one. After re-recording, run `cargo test` to see snapshot diffs, iterate as needed, then accept with `cargo insta accept`.
- Never add `#[allow(...)]` attributes or allow lint rules in `Cargo.toml` without verifying with the user
- Whenever you encounter an issue that's unrelated to what you're currently doing, add it to issues.md so it can be fixed later. This includes if you knowingly add technical debt or skip a requirement. When an issue is resolved, remove it from the list.

## Publishing

Don't publish or release without asking.

1. Bump version in `Cargo.toml` (patch version unless told otherwise)
2. Update CHANGELOG.md and README.md
3. `cargo publish`
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

If a command that should be allowed is denied, or if project structure changes significantly, ask about running `/mats:permissions` to update settings.

## Running Commands

Run git commands plainly from the project root. Don't use `git -C`.

Run builds/tests via `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt`.

**Ad-hoc scripts:** Only `/tmp/claude-execution-allowed/coven/` is approved for ad-hoc scripts. Write bash scripts there and run with `bash /tmp/claude-execution-allowed/coven/<script-name>`.

When you create a new reusable script, offer to add a permission for it. Example: "I created scripts/analyze.sh. Want me to add `Bash(bash scripts/analyze.sh *)` to your permissions?"

For string interpolation, heredocs, loops, or advanced xargs flags, write a script in `/tmp/claude-execution-allowed/coven/` instead.
