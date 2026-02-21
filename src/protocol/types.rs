use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Top-level inbound event from claude's stream-json output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InboundEvent {
    #[serde(rename = "system")]
    System(SystemEvent),
    #[serde(rename = "stream_event")]
    StreamEvent(Box<StreamEvent>),
    #[serde(rename = "assistant")]
    Assistant(AssistantMessage),
    #[serde(rename = "user")]
    User(UserToolResult),
    #[serde(rename = "result")]
    Result(SessionResult),
    #[serde(rename = "rate_limit_event")]
    RateLimit(RateLimitEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype")]
pub enum SystemEvent {
    #[serde(rename = "init")]
    Init(InitEvent),
    #[serde(rename = "status")]
    Status {
        #[serde(default)]
        status: Option<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitEvent {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default, rename = "tools")]
    _tools: Vec<Value>,
}

/// Wrapper for a stream event. The `event` field contains the actual API event payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub event: StreamEventPayload,
    #[serde(flatten)]
    _extra: Value,
}

/// The inner payload of a stream event (the raw Claude API SSE event).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEventPayload {
    #[serde(default, rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub content_block: Option<ContentBlock>,
    #[serde(default)]
    pub delta: Option<Delta>,
    #[serde(flatten)]
    _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, rename = "text")]
    _text: Option<String>,
    #[serde(flatten)]
    _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub partial_json: Option<String>,
    #[serde(default)]
    pub thinking: Option<String>,
    #[serde(default, rename = "stop_reason")]
    _stop_reason: Option<String>,
    #[serde(flatten)]
    _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub message: AssistantMessageBody,
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    #[serde(flatten)]
    _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessageBody {
    #[serde(default)]
    pub content: Vec<AssistantContentBlock>,
    #[serde(flatten)]
    _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AssistantContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "thinking")]
    Thinking {
        #[serde(default, rename = "thinking")]
        _thinking: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserToolResult {
    /// Tool result — can be an object (regular tools), array (MCP tools), or string (errors).
    #[serde(default)]
    pub tool_use_result: Option<Value>,
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    /// Raw message (used for subagent tool results).
    #[serde(default)]
    pub message: Option<Value>,
    #[serde(flatten)]
    _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResult {
    #[serde(default)]
    pub subtype: String,
    #[serde(default)]
    pub total_cost_usd: f64,
    #[serde(default)]
    pub num_turns: u32,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub result: String,
    #[serde(default, rename = "session_id")]
    _session_id: String,
    #[serde(flatten)]
    _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitEvent {
    pub rate_limit_info: RateLimitInfo,
    #[serde(flatten)]
    _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitInfo {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub rate_limit_type: String,
    #[serde(default)]
    pub utilization: f64,
    #[serde(flatten)]
    _extra: Value,
}

impl RateLimitInfo {
    pub fn is_warning(&self) -> bool {
        self.status.contains("warning")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OutboundMessage {
    pub r#type: String,
    pub message: OutboundMessageBody,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutboundMessageBody {
    pub role: String,
    pub content: String,
}
