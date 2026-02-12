pub mod input;
pub mod renderer;
pub mod theme;
pub mod tool_format;

/// Query the current terminal width, defaulting to 80.
pub(crate) fn term_width() -> usize {
    crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
        .max(1)
}
