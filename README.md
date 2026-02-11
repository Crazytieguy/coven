# coven

An oven for claude. A minimal streaming display and workflow runner for Claude Code's `-p` mode.

## Install

```bash
# Homebrew
brew install Crazytieguy/tap/coven

# Shell (macOS/Linux)
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Crazytieguy/coven/releases/latest/download/coven-installer.sh | sh

# Cargo
cargo install coven
```

**Platform support:** macOS and Linux only. Coven uses Unix-specific APIs (terminal control, process signals).

## Why?

The native Claude Code TUI is resource-heavy, blocks on permission prompts, and doesn't support custom workflows. Coven wraps `claude -p --output-format stream-json` to provide a clean, lightweight display with support for follow-up messages, mid-stream steering, and looping workflows.

## Features

- **Streaming display**: One line per tool call, streaming text, collapsed thinking
- **Follow-up messages**: Continue sessions with additional prompts after results
- **Ralph mode**: Loop Claude with fresh sessions, filesystem state persists between iterations
- **Mid-stream steering**: Type messages while Claude is working to redirect it
- **Message inspection**: View full details of any message via `:N` pager command

### Planned

- **Fixed bottom prompt**: Terminal scroll regions for simultaneous output and typing
- **Recurring tasks**: Periodic maintenance tasks (test review, refactoring) as a first-class workflow concept

## Testing

Tests are recorded from real Claude sessions using a VCR approach. Each test case has a `.toml` (config), `.vcr` (recorded NDJSON), and `.snap` (expected display) in `tests/cases/`.

```bash
cargo run --bin record-vcr           # re-record all fixtures
cargo run --bin record-vcr simple_qa # re-record one
cargo insta review                   # review snapshot changes
```

## License

MIT
