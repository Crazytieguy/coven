use serde::{Deserialize, Serialize};

use crate::protocol::types::InboundEvent;

/// Unified application event consumed by the main event loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppEvent {
    /// An event parsed from claude's stdout stream.
    Claude(Box<InboundEvent>),
    /// A warning about an unparseable line from claude's stdout.
    ParseWarning(String),
    /// Content from claude's stderr (displayed as a warning on exit).
    Stderr(String),
    /// The claude process has exited.
    ProcessExit(Option<i32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// Write to stdin immediately (steering).
    Steering,
    /// Buffer until after current result (follow-up).
    FollowUp,
}
