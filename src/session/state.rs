/// Tracks accumulated session state across events.
#[derive(Debug, Default)]
pub struct SessionState {
    pub session_id: Option<String>,
    pub status: SessionStatus,
    pub total_cost_usd: f64,
    /// When true, the next Init event for the same session will skip
    /// rendering the turn separator (`---`). Set when sending a follow-up
    /// so the separator doesn't appear between the follow-up message
    /// and Claude's response.
    pub suppress_next_separator: bool,
    /// User pressed Ctrl+W to request waiting for input after this session
    /// completes, instead of auto-continuing (ralph next iteration, worker
    /// next agent transition).
    pub wait_requested: bool,
    /// Message ID (`msg_...`) of the most recent top-level assistant message.
    /// Used to verify session file persistence before killing the process.
    pub last_message_id: Option<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    #[default]
    Starting,
    Running,
    WaitingForInput,
    Ended,
}
