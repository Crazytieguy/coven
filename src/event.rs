#![allow(dead_code)]

use crate::protocol::types::InboundEvent;

/// Unified application event consumed by the main event loop.
#[derive(Debug)]
pub enum AppEvent {
    /// An event parsed from claude's stdout stream.
    Claude(Box<InboundEvent>),
    /// A warning about an unparseable line from claude's stdout.
    ParseWarning(String),
    /// The claude process has exited.
    ProcessExit(Option<i32>),
    /// User submitted input text.
    UserInput(UserInput),
    /// User requested to view a message by number.
    ViewMessage(usize),
    /// User pressed Ctrl-C.
    Interrupt,
    /// User pressed Ctrl-D (end session gracefully).
    EndSession,
}

#[derive(Debug)]
pub struct UserInput {
    pub text: String,
    pub mode: InputMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Write to stdin immediately (steering).
    Steering,
    /// Buffer until after current result (follow-up).
    FollowUp,
}
