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
coven "explain this codebase"             # single session
coven ralph "pick one task from TODO.md, do it, check it off"
coven init && coven worker                 # multi-agent orchestration from brief.md
```

Pass flags to the underlying `claude` CLI after `--` (e.g. `coven "prompt" -- --resume SESSION_ID`).

## Commands

### `coven [PROMPT]`

Interactive session with streaming display. Supports follow-up messages, mid-stream steering, message inspection (`:N`), and dropping into the native Claude TUI (Ctrl+O).

### `coven ralph <PROMPT>`

Loop Claude: sends the same prompt in fresh sessions until the model outputs a `<break>` tag. The model can output `<wait-for-user>` to pause for human input before continuing; Ctrl+W also triggers a wait after the current turn.

| Flag | Description |
|------|-------------|
| `--iterations N` | Max iterations (0 = infinite, default) |
| `--break-tag TAG` | Custom break tag (default: `break`) |
| `--no-break` | Disable break detection (requires `--iterations`) |
| `--no-wait` | Disable `<wait-for-user>` tag detection |

### `coven worker`

Orchestration worker. Creates a git worktree, runs agents that chain via `<next>` transitions, and sleeps until new commits appear on main.

| Flag | Description |
|------|-------------|
| `--branch NAME` | Worktree branch name (random if omitted) |
| `--worktree-base DIR` | Base directory for worktrees (default: `~/.coven/worktrees`) |
| `--no-wait` | Disable `<wait-for-user>` tag detection (same as ralph) |

### `coven init`

Set up orchestration for a project. Creates `.coven/` directory with agent prompts and config, plus `brief.md` for tasks.

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
- `-- [ARGS]` — pass extra arguments to the claude CLI (e.g. `-- --resume SESSION_ID`)

## Orchestration

`coven init` + `coven worker` enable multi-agent orchestration. Write tasks in `brief.md`, then start workers:

```markdown
# Brief
Choose between two activities:
- Inspect program output across test scenarios and document issues in issues.md
- Pick up an issue from issues.md and do it
```

Workers run a generic agent loop: dispatch reads `brief.md` and picks tasks, then chains through main and review agents via `<next>` transitions. Run multiple workers for parallel execution. See `.coven/system.md` after init for details.

## License

MIT
