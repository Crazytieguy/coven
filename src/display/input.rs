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
    /// User wants to view a message by label (e.g. "3" or "2/1").
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

                // Check for :N or :P/C view command
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

/// Parse `:N` or `:P/C` view commands. Returns the label query (e.g. "3" or "2/1").
fn parse_view_command(text: &str) -> Option<String> {
    let rest = text.trim().strip_prefix(':')?;
    // Accept "N" or "P/C" where N, P, C are positive integers
    if let Some((left, right)) = rest.split_once('/') {
        let p: usize = left.parse().ok()?;
        let c: usize = right.parse().ok()?;
        if p == 0 || c == 0 {
            return None;
        }
        Some(format!("{p}/{c}"))
    } else {
        let n: usize = rest.parse().ok()?;
        if n == 0 {
            return None;
        }
        Some(n.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_view_command_valid() {
        assert_eq!(parse_view_command(":1"), Some("1".to_string()));
        assert_eq!(parse_view_command(":42"), Some("42".to_string()));
        assert_eq!(parse_view_command(": 3"), None); // space not allowed
    }

    #[test]
    fn parse_view_command_slash_notation() {
        assert_eq!(parse_view_command(":2/1"), Some("2/1".to_string()));
        assert_eq!(parse_view_command(":10/3"), Some("10/3".to_string()));
        assert_eq!(parse_view_command(":0/1"), None); // zero not allowed
        assert_eq!(parse_view_command(":1/0"), None); // zero not allowed
        assert_eq!(parse_view_command(":a/1"), None);
    }

    #[test]
    fn parse_view_command_invalid() {
        assert_eq!(parse_view_command("hello"), None);
        assert_eq!(parse_view_command(":abc"), None);
        assert_eq!(parse_view_command(""), None);
    }
}
