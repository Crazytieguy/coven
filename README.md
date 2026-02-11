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

**Platform support:** macOS and Linux only.

## Why?

The native Claude Code TUI is resource-heavy, blocks on permission prompts, and doesn't support custom workflows. Coven wraps `claude -p --output-format stream-json` for a clean, lightweight display with follow-up messages, mid-stream steering, and looping workflows.

## Quick Start

```bash
coven "explain this codebase"             # one-shot prompt
coven                                      # interactive (type prompt at stdin)
coven ralph "fix lint warnings"            # loop until done
coven init && coven worker                 # orchestration
```

## Commands

### `coven [PROMPT]`

Interactive session. Streams tool calls and text, supports follow-ups (Alt+Enter), mid-stream steering (type while running), and message inspection (`:N` to view message N).

### `coven ralph <PROMPT>`

Loop Claude with fresh sessions — filesystem state persists between iterations. The model outputs `<break>reason</break>` to stop.

| Flag | Description |
|------|-------------|
| `--iterations N` | Max iterations (0 = infinite, default) |
| `--break-tag TAG` | Custom break tag (default: `break`) |
| `--no-break` | Disable break detection (requires `--iterations`) |

### `coven worker`

Orchestration worker: dispatch → agent → land loop. Creates a git worktree, picks issues, runs agents, and lands changes.

| Flag | Description |
|------|-------------|
| `--branch NAME` | Worktree branch name (random if omitted) |
| `--worktree-base DIR` | Base directory for worktrees (default: `~/worktrees`) |

### `coven init`

Set up orchestration for a project — creates `.coven/agents/`, `issues/`, and `review/` directories with default prompts.

### `coven status` / `coven gc`

Show active workers / clean up orphaned worktrees.

| Flag | Description |
|------|-------------|
| `--force` | Force removal of dirty worktrees (`gc` only) |

## Shared Flags

All session commands (`coven`, `ralph`, `worker`) accept:

- `--show-thinking` — stream thinking text inline instead of collapsing
- `--fork` — let the model spawn parallel sub-sessions via `<fork>` tags
- `-- [ARGS]` — pass extra arguments to the claude CLI (e.g. `-- --permission-mode plan`)

## Orchestration

`coven init` + `coven worker` enable multi-agent orchestration: workers pick issues, run agent prompts, and land changes on branches. `coven init` generates a `.coven/workflow.md` with full details.

## Testing

Tests use VCR-recorded Claude sessions organized by theme in `tests/cases/{theme}/{name}/`.

```bash
cargo run --bin record-vcr           # re-record all fixtures
cargo run --bin record-vcr simple_qa # re-record one (searches all themes)
cargo insta review                   # review snapshot changes
```

## License

MIT
