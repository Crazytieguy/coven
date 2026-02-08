Issue: Theme colors may need further tuning for specific terminal themes (DarkGrey replaced with Attribute::Dim but worth verifying on light backgrounds too)
Status: draft

## Approach

### We already reuse the terminal theme

crossterm's named colors (`Color::Yellow`, `Color::Red`, etc.) map to ANSI colors 0-15, which are defined by the user's terminal color scheme. If someone uses Solarized Light, their terminal's "Yellow" is already tuned by Solarized to be readable on a light background. Same for iTerm themes, Catppuccin, Dracula, etc.

So the current code already adapts to the terminal theme automatically. Well-designed terminal themes make all 16 ANSI colors readable on their chosen background.

### When this breaks

The problem arises with poorly configured themes or when the user's theme doesn't define readable versions of all 16 colors. Yellow (ANSI 3) is the most common offender — some themes leave it as bright yellow even on white backgrounds.

### Proposed: add color configurability

Since the default ANSI mapping is already theme-adaptive, configurability would serve as an escape hatch for edge cases. The approach:

1. **Add a `[colors]` section to the config TOML** with optional overrides for each semantic role:
   ```toml
   [colors]
   tool_name = "blue"        # default: yellow
   tool_name_dim = "blue"    # default: yellow (dimmed)
   error = "red"             # default: red
   success = "green"         # default: green
   prompt = "cyan"           # default: cyan
   ```

2. **Support ANSI color names and 256-color indices**: Accept the 8 basic names (`black`, `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `white`), their `bright_` variants, and numeric 256-color indices (e.g., `208` for orange).

3. **In `theme.rs`**, change each function to accept an optional color override from config. The simplest way: make `Theme` a struct initialized from config, with a method per style. Falls back to current hardcoded defaults when no override is specified.

4. **Thread the theme through**: `DisplayState` already gets created with config context. Pass the `Theme` struct in and use it where styles are applied.

### Files to change

- `src/config.rs` — add `ColorConfig` struct with optional fields, deserialized from `[colors]`
- `src/display/theme.rs` — convert from free functions to a `Theme` struct with methods
- `src/display/state.rs` — store `Theme` in `DisplayState`, pass it through to rendering
- `src/display/render.rs` — use `state.theme.tool_name()` instead of `theme::tool_name()`
- Update any other call sites of `theme::*` functions

### Not included

- No auto-detection of light/dark background (unreliable, requires terminal escape sequence query with timeout)
- No built-in alternate theme presets (YAGNI — the override system covers this)

## Questions

### Should we support RGB hex colors (e.g., `#ff8800`)?

crossterm supports `Color::Rgb { r, g, b }` which would allow exact color specification. This is more powerful but won't adapt to terminal themes the way ANSI colors do.

Options:
1. **ANSI names + 256-color only** — keeps colors theme-adaptive, simpler to document
2. **Also support `#rrggbb` hex** — more flexible, useful for users with truecolor terminals

Answer:

## Review
