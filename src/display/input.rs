use std::io::{self, Write};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::{cursor, queue, terminal};

use crate::event::InputMode;

/// Result of processing a key event.
pub enum InputAction {
    /// No action yet — still editing.
    None,
    /// User submitted text (Enter = steering, Alt+Enter = follow-up).
    Submit(String, InputMode),
    /// User wants to view message N.
    ViewMessage(usize),
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
                    // Start input mode
                    self.activate();
                    self.buffer.push(c);
                    // Echo the character
                    let mut out = io::stdout();
                    queue!(out, crossterm::style::Print(c)).ok();
                    out.flush().ok();
                    return InputAction::None;
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

                let mut out = io::stdout();
                queue!(out, crossterm::style::Print("\r\n")).ok();
                out.flush().ok();

                if text.is_empty() {
                    return InputAction::None;
                }

                // Check for :N view command
                if let Some(n) = parse_view_command(&text) {
                    return InputAction::ViewMessage(n);
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

    /// Render the prompt and any current buffer content.
    pub fn render_prompt_with_buffer(&self) {
        let mut out = io::stdout();
        queue!(
            out,
            crossterm::style::Print(super::theme::prompt_style().apply("> ")),
        )
        .ok();
        if !self.buffer.is_empty() {
            queue!(out, crossterm::style::Print(&self.buffer)).ok();
        }
        out.flush().ok();
    }
}

fn parse_view_command(text: &str) -> Option<usize> {
    text.trim().strip_prefix(':')?.parse::<usize>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_view_command_valid() {
        assert_eq!(parse_view_command(":1"), Some(1));
        assert_eq!(parse_view_command(":42"), Some(42));
        assert_eq!(parse_view_command(": 3"), None); // space not allowed
    }

    #[test]
    fn parse_view_command_invalid() {
        assert_eq!(parse_view_command("hello"), None);
        assert_eq!(parse_view_command(":abc"), None);
        assert_eq!(parse_view_command(""), None);
    }
}
