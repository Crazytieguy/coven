use std::io::{self, Write};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::{cursor, queue, terminal};
use unicode_width::UnicodeWidthStr;

use crate::event::InputMode;

fn term_width() -> usize {
    terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80)
        .max(1)
}

/// Result of processing a key event.
pub enum InputAction {
    /// No action yet — still editing.
    None,
    /// First character typed while inactive — caller should set up input line.
    Activated(char),
    /// User submitted text (Enter = steering, Alt+Enter = follow-up).
    Submit(String, InputMode),
    /// User wants to view a message (e.g. ":3", ":2/1", ":Bash", ":Edit[-1]").
    ViewMessage(String),
    /// User cancelled input (Escape).
    Cancel,
    /// User pressed Ctrl-C.
    Interrupt,
    /// User pressed Ctrl-D.
    EndSession,
}

/// Simple line editor for user input in raw mode.
pub struct InputHandler {
    buffer: String,
    active: bool,
    prefix_width: usize,
}

impl InputHandler {
    pub fn new(prefix_width: usize) -> Self {
        Self {
            buffer: String::new(),
            active: false,
            prefix_width,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activate the input handler (show prompt, start accepting keys).
    pub fn activate(&mut self) {
        self.buffer.clear();
        self.active = true;
    }

    /// Deactivate without clearing — used after submit/cancel.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.buffer.clear();
    }

    /// Clear all terminal lines occupied by the input (prefix + buffer),
    /// accounting for line wrapping at the terminal width.
    fn clear_input_lines(&self, buffer: &str) {
        let mut out = io::stdout();
        let tw = term_width();
        let input_display_width = buffer.width() + self.prefix_width;
        let lines_occupied = input_display_width.div_ceil(tw).max(1);
        if lines_occupied > 1 {
            queue!(
                out,
                cursor::MoveUp(u16::try_from(lines_occupied - 1).unwrap_or(u16::MAX))
            )
            .ok();
        }
        queue!(
            out,
            crossterm::style::Print("\r"),
            terminal::Clear(terminal::ClearType::FromCursorDown),
        )
        .ok();
        out.flush().ok();
    }

    /// Process a terminal key event. Returns the action to take.
    pub fn handle_key(&mut self, event: &KeyEvent) -> InputAction {
        if !self.active {
            // If not active, check for character to start input
            match event.code {
                KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                    return InputAction::Interrupt;
                }
                KeyCode::Char('d') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                    return InputAction::EndSession;
                }
                KeyCode::Char(c) => {
                    // Start input mode — caller handles the visual setup
                    self.activate();
                    self.buffer.push(c);
                    return InputAction::Activated(c);
                }
                _ => return InputAction::None,
            }
        }

        match event.code {
            KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                InputAction::Interrupt
            }
            KeyCode::Char('d') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                InputAction::EndSession
            }
            KeyCode::Char(c) => {
                self.buffer.push(c);
                let mut out = io::stdout();
                queue!(out, crossterm::style::Print(c)).ok();
                out.flush().ok();
                InputAction::None
            }
            KeyCode::Backspace => {
                if !self.buffer.is_empty() {
                    let old_display_width = self.prefix_width + self.buffer.width();
                    self.buffer.pop();
                    let mut out = io::stdout();
                    let tw = term_width();
                    if old_display_width.is_multiple_of(tw) {
                        // Cursor is at column 0 of a wrapped line — move up
                        queue!(
                            out,
                            cursor::MoveUp(1),
                            cursor::MoveToColumn(u16::try_from(tw - 1).unwrap_or(u16::MAX)),
                            terminal::Clear(terminal::ClearType::FromCursorDown),
                        )
                        .ok();
                    } else {
                        queue!(
                            out,
                            cursor::MoveLeft(1),
                            terminal::Clear(terminal::ClearType::FromCursorDown),
                        )
                        .ok();
                    }
                    out.flush().ok();
                }
                InputAction::None
            }
            KeyCode::Enter => {
                let text = self.buffer.clone();
                self.deactivate();
                self.clear_input_lines(&text);

                if text.is_empty() {
                    return InputAction::None;
                }

                // Check for view command (:N, :P/C, :Label, :Label[index])
                if let Some(query) = parse_view_command(&text) {
                    return InputAction::ViewMessage(query);
                }

                let mode = if event.modifiers.contains(KeyModifiers::ALT) {
                    InputMode::FollowUp
                } else {
                    InputMode::Steering
                };

                InputAction::Submit(text, mode)
            }
            KeyCode::Esc => {
                let text = self.buffer.clone();
                self.deactivate();
                self.clear_input_lines(&text);
                InputAction::Cancel
            }
            _ => InputAction::None,
        }
    }
}

/// Parse view commands. Returns the label query string.
///
/// Accepted forms:
/// - `:N` or `:P/C` — numeric (e.g. `:3` → `"3"`, `:2/1` → `"2/1"`)
/// - `:Label` or `:Label[index]` — label-based (e.g. `:Bash` → `"Bash"`, `:Edit[-1]` → `"Edit[-1]"`)
fn parse_view_command(text: &str) -> Option<String> {
    let rest = text.trim().strip_prefix(':')?;
    if rest.is_empty() {
        return None;
    }

    // Numeric: "N" or "P/C"
    if let Some((left, right)) = rest.split_once('/')
        && let (Ok(p), Ok(c)) = (left.parse::<usize>(), right.parse::<usize>())
    {
        if p > 0 && c > 0 {
            return Some(format!("{p}/{c}"));
        }
        return None;
    }
    if let Ok(n) = rest.parse::<usize>() {
        return if n > 0 { Some(n.to_string()) } else { None };
    }

    // Label-based: validate Name or Name[index]
    let (name, tail) = if let Some(bracket) = rest.find('[') {
        let after = rest[bracket + 1..].strip_suffix(']')?;
        let _: isize = after.parse().ok()?;
        (&rest[..bracket], &rest[bracket..])
    } else {
        (rest, "")
    };

    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | ':' | '_'))
    {
        return None;
    }

    Some(format!("{name}{tail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_view_command_numeric() {
        assert_eq!(parse_view_command(":1"), Some("1".to_string()));
        assert_eq!(parse_view_command(":42"), Some("42".to_string()));
        assert_eq!(parse_view_command(":0"), None);
        assert_eq!(parse_view_command(": 3"), None); // space not allowed
    }

    #[test]
    fn parse_view_command_slash_notation() {
        assert_eq!(parse_view_command(":2/1"), Some("2/1".to_string()));
        assert_eq!(parse_view_command(":10/3"), Some("10/3".to_string()));
        assert_eq!(parse_view_command(":0/1"), None);
        assert_eq!(parse_view_command(":1/0"), None);
        assert_eq!(parse_view_command(":a/1"), None);
    }

    #[test]
    fn parse_view_command_label() {
        assert_eq!(parse_view_command(":Bash"), Some("Bash".to_string()));
        assert_eq!(parse_view_command(":Bash[0]"), Some("Bash[0]".to_string()));
        assert_eq!(
            parse_view_command(":Edit[-1]"),
            Some("Edit[-1]".to_string())
        );
        assert_eq!(
            parse_view_command(":Thinking[2]"),
            Some("Thinking[2]".to_string())
        );
        // MCP tool names with hyphens and colons
        assert_eq!(
            parse_view_command(":llms-fetch:fetch"),
            Some("llms-fetch:fetch".to_string())
        );
    }

    #[test]
    fn parse_view_command_invalid() {
        assert_eq!(parse_view_command("hello"), None);
        assert_eq!(parse_view_command(""), None);
        assert_eq!(parse_view_command(":"), None);
        assert_eq!(parse_view_command(":[0]"), None); // empty name
        assert_eq!(parse_view_command(":Bash[abc]"), None); // non-integer index
    }
}
