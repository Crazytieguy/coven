use std::io::Write;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::{cursor, queue, terminal};
use unicode_width::UnicodeWidthStr;

use super::term_width;
use super::theme;
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
    /// User cancelled input (Escape with text in buffer).
    Cancel,
    /// User dismissed the prompt (Escape on empty buffer).
    Dismiss,
    /// User pressed Ctrl-C.
    Interrupt,
    /// User pressed Ctrl-D.
    EndSession,
    /// User wants to drop into native Claude TUI.
    Interactive,
    /// User pressed Ctrl+W to toggle wait-for-input after session completes.
    WaitRequested,
}

/// Simple line editor for user input in raw mode.
pub struct InputHandler {
    buffer: String,
    /// Cursor position as a char index into the buffer.
    cursor: usize,
    active: bool,
    prefix_width: usize,
    /// Tracks where the terminal cursor currently is (display columns from
    /// the start of the input line, including the prefix). Used by `redraw()`
    /// to navigate back to the beginning before reprinting.
    term_cursor_display: usize,
    /// Whether a hint line was rendered above the current input line.
    /// When set, `clear_input_lines` moves up one extra line to erase it.
    has_hint_line: bool,
}

impl InputHandler {
    pub fn new(prefix_width: usize) -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            active: false,
            prefix_width,
            term_cursor_display: 0,
            has_hint_line: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activate the input handler (show prompt, start accepting keys).
    pub fn activate(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.active = true;
        self.term_cursor_display = self.prefix_width;
        self.has_hint_line = false;
    }

    /// Mark that a hint line was rendered above the current input line.
    pub fn set_has_hint_line(&mut self) {
        self.has_hint_line = true;
    }

    /// Deactivate without clearing — used after submit/cancel.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.buffer.clear();
        self.cursor = 0;
    }

    /// Byte offset in the buffer corresponding to the current char-index cursor.
    fn cursor_byte_pos(&self) -> usize {
        self.byte_pos_at(self.cursor)
    }

    /// Char index of the nearest word boundary to the left of the cursor.
    fn word_boundary_left(&self) -> usize {
        let chars: Vec<char> = self.buffer.chars().collect();
        let mut i = self.cursor;
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        i
    }

    /// Char index of the nearest word boundary to the right of the cursor.
    fn word_boundary_right(&self) -> usize {
        let chars: Vec<char> = self.buffer.chars().collect();
        let len = chars.len();
        let mut i = self.cursor;
        while i < len && !chars[i].is_whitespace() {
            i += 1;
        }
        while i < len && chars[i].is_whitespace() {
            i += 1;
        }
        i
    }

    /// Redraw the entire input line and position the cursor.
    ///
    /// Call this after any buffer or cursor modification. Uses
    /// `term_cursor_display` to navigate to the start of the input,
    /// then reprints the prefix and buffer, clears leftover content,
    /// and moves the cursor to the correct column.
    pub fn redraw(&mut self, out: &mut impl Write) {
        let tw = term_width();

        // Move to start of input region
        let cur_line = self.term_cursor_display / tw;
        if cur_line > 0 {
            queue!(
                out,
                cursor::MoveUp(u16::try_from(cur_line).unwrap_or(u16::MAX))
            )
            .ok();
        }
        queue!(out, crossterm::style::Print("\r")).ok();

        // Redraw prefix + buffer
        let total_display = self.prefix_width + self.buffer.width();

        queue!(
            out,
            crossterm::style::Print(theme::prompt_style().apply("> ")),
            crossterm::style::Print(&self.buffer),
        )
        .ok();

        // When total display width is an exact multiple of terminal width, the
        // terminal cursor is in "pending wrap" state rather than on the next line.
        // Print a space to force the wrap to resolve, then move back.
        if total_display > 0 && total_display.is_multiple_of(tw) {
            queue!(out, crossterm::style::Print(" "), cursor::MoveLeft(1)).ok();
        }

        queue!(out, terminal::Clear(terminal::ClearType::FromCursorDown)).ok();

        // Move terminal cursor from end-of-buffer to the actual cursor position
        let byte_pos = self.cursor_byte_pos();
        let new_cursor_display = self.prefix_width + self.buffer[..byte_pos].width();
        let end_line = total_display / tw;
        let target_line = new_cursor_display / tw;
        let target_col = new_cursor_display % tw;

        let lines_up = end_line.saturating_sub(target_line);
        if lines_up > 0 {
            queue!(
                out,
                cursor::MoveUp(u16::try_from(lines_up).unwrap_or(u16::MAX))
            )
            .ok();
        }
        queue!(
            out,
            cursor::MoveToColumn(u16::try_from(target_col).unwrap_or(u16::MAX))
        )
        .ok();
        out.flush().ok();

        self.term_cursor_display = new_cursor_display;
    }

    /// Clear all terminal lines occupied by the input (prefix + buffer),
    /// accounting for line wrapping at the terminal width.
    /// Also clears the hint line above the input if one was rendered.
    fn clear_input_lines(&self, out: &mut impl Write) {
        let tw = term_width();
        // Use term_cursor_display to find which line the cursor is on,
        // then move to the start of the input region.
        let mut lines_up = self.term_cursor_display / tw;
        // If a hint line was rendered above the input, include it in the clear.
        if self.has_hint_line {
            lines_up += 1;
        }
        if lines_up > 0 {
            queue!(
                out,
                cursor::MoveUp(u16::try_from(lines_up).unwrap_or(u16::MAX))
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

    /// Move the cursor to `pos` and redraw.
    fn move_cursor(&mut self, pos: usize, out: &mut impl Write) {
        self.cursor = pos;
        self.redraw(out);
    }

    /// Insert a character at the cursor position and redraw.
    fn insert_char(&mut self, c: char, out: &mut impl Write) {
        let byte_pos = self.cursor_byte_pos();
        self.buffer.insert(byte_pos, c);
        self.cursor += 1;
        self.redraw(out);
    }

    /// Delete chars in `[from_char..to_char)`, set cursor to `from_char`, and redraw.
    fn delete_range(&mut self, from_char: usize, to_char: usize, out: &mut impl Write) {
        let from_byte = self.byte_pos_at(from_char);
        let to_byte = self.byte_pos_at(to_char);
        self.buffer.drain(from_byte..to_byte);
        self.cursor = from_char;
        self.redraw(out);
    }

    /// Byte offset for a given char index.
    fn byte_pos_at(&self, char_idx: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(char_idx)
            .map_or(self.buffer.len(), |(i, _)| i)
    }

    /// Process a terminal key event. Returns the action to take.
    pub fn handle_key(&mut self, event: &KeyEvent, out: &mut impl Write) -> InputAction {
        if !self.active {
            return self.handle_inactive_key(event, out);
        }

        let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
        let alt = event.modifiers.contains(KeyModifiers::ALT);
        let len = self.buffer.chars().count();

        match event.code {
            KeyCode::Char('c') if ctrl => InputAction::Interrupt,
            KeyCode::Char('d') if ctrl => InputAction::EndSession,
            KeyCode::Char('o') if ctrl => InputAction::Interactive,

            KeyCode::Left if ctrl || alt => {
                self.move_cursor(self.word_boundary_left(), out);
                InputAction::None
            }
            KeyCode::Right if ctrl || alt => {
                self.move_cursor(self.word_boundary_right(), out);
                InputAction::None
            }
            KeyCode::Char('b') if alt => {
                self.move_cursor(self.word_boundary_left(), out);
                InputAction::None
            }
            KeyCode::Char('f') if alt => {
                self.move_cursor(self.word_boundary_right(), out);
                InputAction::None
            }
            KeyCode::Left if self.cursor > 0 => {
                self.move_cursor(self.cursor - 1, out);
                InputAction::None
            }
            KeyCode::Right if self.cursor < len => {
                self.move_cursor(self.cursor + 1, out);
                InputAction::None
            }
            KeyCode::Char('a') if ctrl => {
                self.move_cursor(0, out);
                InputAction::None
            }
            KeyCode::Char('e') if ctrl => {
                self.move_cursor(len, out);
                InputAction::None
            }
            KeyCode::Home => {
                self.move_cursor(0, out);
                InputAction::None
            }
            KeyCode::End => {
                self.move_cursor(len, out);
                InputAction::None
            }

            KeyCode::Backspace if alt => {
                let t = self.word_boundary_left();
                self.delete_range(t, self.cursor, out);
                InputAction::None
            }
            KeyCode::Char('w') if ctrl => {
                let t = self.word_boundary_left();
                self.delete_range(t, self.cursor, out);
                InputAction::None
            }
            KeyCode::Char('u') if ctrl => {
                self.delete_range(0, self.cursor, out);
                InputAction::None
            }
            KeyCode::Char('k') if ctrl => {
                self.delete_range(self.cursor, len, out);
                InputAction::None
            }
            KeyCode::Char('d') if alt => {
                let t = self.word_boundary_right();
                self.delete_range(self.cursor, t, out);
                InputAction::None
            }
            KeyCode::Delete if self.cursor < len => {
                self.delete_range(self.cursor, self.cursor + 1, out);
                InputAction::None
            }

            KeyCode::Char(c) => {
                self.insert_char(c, out);
                InputAction::None
            }

            KeyCode::Backspace if self.cursor > 0 => {
                self.delete_range(self.cursor - 1, self.cursor, out);
                InputAction::None
            }

            KeyCode::Enter => self.handle_enter(event, out),
            KeyCode::Esc => {
                let was_empty = self.buffer.is_empty();
                self.deactivate();
                self.clear_input_lines(out);
                if was_empty {
                    InputAction::Dismiss
                } else {
                    InputAction::Cancel
                }
            }

            _ => InputAction::None,
        }
    }

    fn handle_inactive_key(&mut self, event: &KeyEvent, _out: &mut impl Write) -> InputAction {
        let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
        match event.code {
            KeyCode::Char('c') if ctrl => InputAction::Interrupt,
            KeyCode::Char('d') if ctrl => InputAction::EndSession,
            KeyCode::Char('o') if ctrl => InputAction::Interactive,
            KeyCode::Char('w') if ctrl => InputAction::WaitRequested,
            KeyCode::Char(c) => {
                // Activate and buffer the character, but don't redraw yet.
                // The caller will call begin_input_line() to set up a fresh
                // line before redrawing.
                self.activate();
                let byte_pos = self.cursor_byte_pos();
                self.buffer.insert(byte_pos, c);
                self.cursor += 1;
                InputAction::Activated(c)
            }
            _ => InputAction::None,
        }
    }

    fn handle_enter(&mut self, event: &KeyEvent, out: &mut impl Write) -> InputAction {
        let text = self.buffer.clone();
        self.deactivate();
        self.clear_input_lines(out);

        if text.is_empty() {
            return InputAction::None;
        }

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
