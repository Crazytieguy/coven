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
- Never write `.vcr` files directly — they must only be created or modified by `cargo run --bin record-vcr`. Re-recording is cheap enough: `cargo run --bin record-vcr` re-records all fixtures, `cargo run --bin record-vcr simple_qa` re-records one. After re-recording, run `cargo test` to see snapshot diffs, iterate as needed, then accept with `cargo insta accept`.
- Always prefer properly VCR-recording I/O operations over working around them. Every external I/O call (filesystem, process info, network, etc.) should go through `vcr.call()` so it's recorded during recording and replayed deterministically during tests. Never use `vcr.is_live()`/`vcr.is_replay()` guards to skip I/O — instead, wrap the I/O in a VCR call.
- Never add `#[allow(...)]` attributes or allow lint rules in `Cargo.toml` without verifying with the user
- Always record issues you encounter that are unrelated to your current work — add them as one-liners to issues.md so they can be planned and fixed later. This includes bugs you notice, UI problems, technical debt you knowingly add, requirements you skip, and improvements you spot. Don't let things slip through the cracks. When an issue is resolved, remove it from the list.
- Never make security-relevant decisions without confirmation. This includes permission modes, authentication, access control, and anything that affects the trust boundary of the system. Always apply least-privilege: when granting permissions to spawned agents (e.g. in test fixtures), allow only the specific commands needed, never broad wildcards like `Bash(*)`.
- See workflow.md for the autonomous ralph-mode workflow (plan-based issue tracking, priorities, session discipline).

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
