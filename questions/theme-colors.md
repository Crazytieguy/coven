Blocks: Theme colors may need further tuning for specific terminal themes (DarkGrey replaced with Attribute::Dim but worth verifying on light backgrounds too)

## What's the scope of this issue?

The current color palette is:
- **Yellow** — tool call names (`tool_name()`, `tool_name_dim()`)
- **Red** — errors
- **Green + Bold** — result/done line
- **Cyan + Bold** — input prompt
- **Dim (no color)** — secondary text (help, session headers, stats, thinking labels)

The DarkGrey-on-dark-background problem was already fixed by switching to `Attribute::Dim`. The remaining concern is light backgrounds. On most terminals:
- `Attribute::Dim` on a light background darkens text, which should be fine
- Standard ANSI Yellow on a white background can be hard to read (it's often rendered as olive/brown but some terminals make it genuinely light yellow)
- Dim + Yellow (`tool_name_dim()` for subagent tools) would be even harder to read on light backgrounds

Options:
1. **Close as-is** — the current palette uses standard ANSI colors and Dim, which is about as portable as you can get. Users with problematic terminal themes can adjust their terminal's color definitions.
2. **Verify and close** — you try it on a light terminal theme, confirm it's acceptable (or not), and we act on what you see.
3. **Replace Yellow** — swap tool name color to something more universally readable (e.g., Blue or Magenta). This would change the look on dark terminals too.
4. **Add theme detection** — use something like `termbg` crate to detect light/dark background and adjust colors accordingly. More robust but adds complexity and a dependency.

Answer:
