Issue: Theme colors may need further tuning for specific terminal themes (DarkGrey replaced with Attribute::Dim but worth verifying on light backgrounds too)
Status: draft

## Approach

### Current State

All styling is centralized in `src/display/theme.rs` with 7 style functions using crossterm's `ContentStyle`. The palette is:
- **Yellow**: Tool call names (bright and dimmed variants)
- **Red**: Error indicators/messages
- **Green + Bold**: Success "Done" label
- **Cyan + Bold**: User input prompt `>`
- **Dim**: Metadata, stats, help text, warnings
- **Dim + Italic**: Thinking blocks

DarkGrey was previously replaced with `Attribute::Dim` — this is the safer choice since `Dim` adapts to the terminal's foreground color rather than picking an absolute color value.

### Potential Problems on Light Backgrounds

1. **Yellow** (`Color::Yellow`) — this is the biggest risk. On light/white backgrounds, yellow text is notoriously hard to read. Tool call names (`[N] > tool_name`) use this and are prominent UI elements.
2. **Green** (`Color::Green`) — can be low-contrast on some light themes, though Bold helps.
3. **Cyan** (`Color::Cyan`) — generally readable on both, Bold helps further.
4. **Dim** — adapts to foreground, should be fine on both.
5. **Red** — readable on both dark and light backgrounds.

### Proposed Changes

Replace `Color::Yellow` with `Color::DarkYellow` (or a similar darker variant) for tool names. This reads better on light backgrounds while remaining visible on dark ones. Alternatively, consider `Color::Magenta` or `Color::Blue` which have good contrast on both.

However, this is a judgment call that really needs manual verification on actual terminals.

## Questions

### What level of light-theme support do you want?

The simplest fix is swapping Yellow for a more universally-readable color (like Blue or Magenta for tool names). A more thorough approach would be testing every style on 2-3 common terminal themes (default dark, Solarized Light, macOS default light) and tuning individually.

Options:
1. **Targeted fix**: Just swap Yellow to something safer (e.g., Blue or Magenta) since it's the most problematic color. Quick, low-risk.
2. **Full audit**: Manually test on light terminals and tune all colors as needed. More thorough but requires your visual feedback.
3. **Terminal-adaptive theming**: Detect background color and switch palettes. Most robust but significantly more complex.
4. **Close as won't-fix**: The current scheme works well on dark backgrounds (the common case). Light-theme users are a small minority for CLI tools.

Answer:

## Review

