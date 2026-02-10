use std::io::{self, Write};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::{cursor, queue, terminal};

use crate::event::InputMode;

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
#[derive(Default)]
pub struct InputHandler {
    buffer: String,
    active: bool,
}

impl InputHandler {
    pub fn new() -> Self {
        Self::default()
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
                    self.buffer.pop();
                    let mut out = io::stdout();
                    // Move back, clear to end of line
                    queue!(
                        out,
                        cursor::MoveLeft(1),
                        terminal::Clear(terminal::ClearType::UntilNewLine),
                    )
                    .ok();
                    out.flush().ok();
                }
                InputAction::None
            }
            KeyCode::Enter => {
                let text = self.buffer.clone();
                self.deactivate();

                // Clear the input line so buffered output can print in its place
                let mut out = io::stdout();
                queue!(
                    out,
                    crossterm::style::Print("\r"),
                    terminal::Clear(terminal::ClearType::CurrentLine),
                )
                .ok();
                out.flush().ok();

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
                self.deactivate();
                // Clear the input line
                let mut out = io::stdout();
                queue!(
                    out,
                    crossterm::style::Print("\r"),
                    terminal::Clear(terminal::ClearType::CurrentLine),
                )
                .ok();
                out.flush().ok();
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
