# coven

An oven for claude. A minimal streaming display and workflow runner for Claude Code's `-p` mode.

## Status

Early development - project scaffolding phase.

## Features (Planned)

- **Minimal streaming display**: Run `claude -p` with streaming JSON output, parse and display with a clean, configurable TUI
- **Hook-based communication**: Send messages to Claude via Claude Code hooks
- **Session continuation**: Follow up on ended sessions with `claude --continue`
- **Ralph Wiggum looping**: Run the same prompt repeatedly with filesystem state accumulation
- **Session inspector**: View full details of tool calls and session activity

## Why `-p` mode?

- **Performance**: Each Claude TUI instance is expensive; `-p` mode is lightweight for parallel runs
- **Permissions**: Claude keeps going instead of blocking on permission prompts
- **Flexibility**: Enables workflows not natively supported by Claude Code

## Installation

```
cargo install --path .
```

## Usage

TODO

## License

TODO
