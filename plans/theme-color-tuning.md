Issue: Theme colors may need further tuning for specific terminal themes (DarkGrey replaced with Attribute::Dim but worth verifying on light backgrounds too)
Status: draft

## Approach

The current color choices (Yellow, Red, Green, Cyan) are all standard ANSI colors 0-7 which map to the user's terminal color scheme. This is already the right approach — well-designed terminal themes define readable versions of all 16 ANSI colors.

### Changes

1. **Add a module-level doc comment to `src/display/theme.rs`** explaining the design constraint:
   - We use only named ANSI colors (not `Color::Rgb`, `Color::AnsiValue`, or bright variants) so that colors adapt to the user's terminal theme.
   - If adding new styles, stick to the 8 basic ANSI colors: Black, Red, Green, Yellow, Blue, Magenta, Cyan, White.
   - Use `Attribute::Dim` / `Attribute::Bold` for emphasis rather than bright color variants.

That's it — the code is already correct and well-centralized. This just documents the constraint for future contributors.

### Files to change

- `src/display/theme.rs` — add module doc comment

## Questions

None.

## Review

> I didn't know ANSI colors worked like this! In that case, let's just make sure we only use colors that are likely to work across a variety of terminal configurations, document this constraint wherever in the code we set the colors (should be centralized), and move on.
