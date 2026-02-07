#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Top-level inbound event from claude's stream-json output.
#[derive(Debug, Clone, Deserialize)]
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
}

// --- System events ---

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "subtype")]
pub enum SystemEvent {
    #[serde(rename = "init")]
    Init(InitEvent),
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InitEvent {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub tools: Vec<Value>,
}

// --- Stream events (raw API streaming) ---

#[derive(Debug, Clone, Deserialize)]
pub struct StreamEvent {
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub content_block: Option<ContentBlock>,
    #[serde(default)]
    pub delta: Option<Delta>,
    /// Catch-all for fields we don't explicitly model.
    #[serde(flatten)]
    pub extra: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContentBlock {
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(flatten)]
    pub extra: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Delta {
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub partial_json: Option<String>,
    #[serde(default)]
    pub thinking: Option<String>,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(flatten)]
    pub extra: Value,
}

// --- Assistant message (complete) ---

#[derive(Debug, Clone, Deserialize)]
pub struct AssistantMessage {
    pub message: AssistantMessageBody,
    #[serde(flatten)]
    pub extra: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssistantMessageBody {
    #[serde(default)]
    pub content: Vec<AssistantContentBlock>,
    #[serde(flatten)]
    pub extra: Value,
}

#[derive(Debug, Clone, Deserialize)]
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
        #[serde(default)]
        thinking: String,
    },
    #[serde(other)]
    Other,
}

// --- User (tool result) ---

#[derive(Debug, Clone, Deserialize)]
pub struct UserToolResult {
    #[serde(default)]
    pub tool_use_result: Option<ToolUseResult>,
    #[serde(flatten)]
    pub extra: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ToolUseResult {
    #[serde(default)]
    pub tool_use_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub is_error: bool,
    #[serde(flatten)]
    pub extra: Value,
}

// --- Result ---

#[derive(Debug, Clone, Deserialize)]
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
    #[serde(default)]
    pub session_id: String,
    #[serde(flatten)]
    pub extra: Value,
}

// --- Outbound messages ---

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
