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

Interactive session. Streams tool calls and text, supports follow-ups (Alt+Enter), mid-stream steering (type while running), message inspection (`:N` to view message N), wait-for-input marking (Ctrl+W, for ralph/worker), native TUI access (Ctrl+O), and end session (Ctrl+D).

### `coven ralph <PROMPT>`

Loop Claude with fresh sessions — filesystem state persists between iterations. The model outputs `<break>reason</break>` to stop.

| Flag | Description |
|------|-------------|
| `--iterations N` | Max iterations (0 = infinite, default) |
| `--break-tag TAG` | Custom break tag (default: `break`) |
| `--no-break` | Disable break detection (requires `--iterations`) |
| `--no-wait` | Disable `<wait-for-user>` tag detection |

### `coven worker`

Orchestration worker: generic agent loop. Creates a git worktree, runs agents that chain via `<next>` transitions, and sleeps when idle.

| Flag | Description |
|------|-------------|
| `--branch NAME` | Worktree branch name (random if omitted) |
| `--worktree-base DIR` | Base directory for worktrees (default: `~/.coven/worktrees`) |
| `--no-wait` | Disable `<wait-for-user>` tag detection |

### `coven init`

Set up orchestration for a project — creates `.coven/` directory (config, agent prompts, system doc, land script) and `brief.md`.

### `coven status` / `coven gc`

Show active workers / clean up orphaned worktrees.

| Flag | Description |
|------|-------------|
| `--force` | Force removal of dirty worktrees (`gc` only) |

## Shared Flags

All session commands (`coven`, `ralph`, `worker`) accept:

- `--show-thinking` — stream thinking text inline instead of collapsing
- `--fork` — let the model spawn parallel sub-sessions via `<fork>` tags
- `--reload` — let the model reload claude via `<reload>` tags (preserves session)
- `-- [ARGS]` — pass extra arguments to the claude CLI (e.g. `-- --permission-mode plan`)

## Orchestration

`coven init` + `coven worker` enable multi-agent orchestration. Workers run a generic agent loop: dispatch reads `brief.md` and picks tasks, then chains through main and review agents via `<next>` transitions. See `.coven/system.md` after init for details.

## License

MIT
