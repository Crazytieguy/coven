use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::session::runner::{SessionConfig, SessionRunner};

/// Test case definition loaded from a `.toml` file.
#[derive(Deserialize, Default)]
pub struct TestCase {
    /// Configuration for a standard run.
    pub run: Option<RunConfig>,
    /// Configuration for ralph loop mode.
    pub ralph: Option<RalphConfig>,
    /// Files to create in the working directory before recording.
    #[serde(default)]
    pub files: HashMap<String, String>,
    /// Additional messages to send during the session (follow-ups, steering).
    #[serde(default)]
    pub messages: Vec<TestMessage>,
}

/// CLI configuration for a standard run (mirrors coven's CLI args).
#[derive(Deserialize)]
pub struct RunConfig {
    /// Prompt to send to claude.
    pub prompt: String,
    /// Extra arguments to pass through to claude.
    #[serde(default)]
    pub claude_args: Vec<String>,
}

/// CLI configuration for ralph loop mode (mirrors coven's ralph subcommand args).
#[derive(Deserialize)]
pub struct RalphConfig {
    /// Prompt to send on each iteration.
    pub prompt: String,
    /// Tag that signals loop completion.
    #[serde(default = "default_break_tag")]
    pub break_tag: String,
    /// Extra arguments to pass through to claude.
    #[serde(default)]
    pub claude_args: Vec<String>,
}

fn default_break_tag() -> String {
    "break".to_string()
}

/// A message to send during a recording session.
#[derive(Deserialize)]
pub struct TestMessage {
    /// The message content.
    pub content: String,
    /// When to send: "after-result", "after-tool:N", "after-message:N".
    pub trigger: String,
}

/// VCR file header — first line of every `.vcr` file.
#[derive(Deserialize, Serialize)]
pub struct VcrHeader {
    pub _vcr: String,
    pub command: Vec<String>,
}

/// Trigger types parsed from TestMessage.trigger strings.
pub enum Trigger {
    AfterResult,
    AfterTool(usize),
    AfterMessage(usize),
}

impl TestCase {
    /// Build a SessionConfig from this test case.
    pub fn session_config(&self) -> SessionConfig {
        if let Some(ref ralph) = self.ralph {
            SessionConfig {
                prompt: Some(ralph.prompt.clone()),
                extra_args: ralph.claude_args.clone(),
                append_system_prompt: Some(SessionRunner::ralph_system_prompt(&ralph.break_tag)),
            }
        } else if let Some(ref run) = self.run {
            SessionConfig {
                prompt: Some(run.prompt.clone()),
                extra_args: run.claude_args.clone(),
                append_system_prompt: None,
            }
        } else {
            panic!("Test case must have either [run] or [ralph] section");
        }
    }

    /// Build the expected CLI command (including "claude" prefix).
    pub fn expected_command(&self) -> Vec<String> {
        let config = self.session_config();
        let mut cmd = vec!["claude".to_string()];
        cmd.extend(SessionRunner::build_args(&config));
        cmd
    }

    /// Get the initial prompt.
    pub fn prompt(&self) -> &str {
        if let Some(ref ralph) = self.ralph {
            &ralph.prompt
        } else if let Some(ref run) = self.run {
            &run.prompt
        } else {
            panic!("Test case must have either [run] or [ralph] section");
        }
    }

    /// Whether this is a ralph test case.
    pub fn is_ralph(&self) -> bool {
        self.ralph.is_some()
    }

    /// Get the ralph break tag (if ralph mode).
    pub fn break_tag(&self) -> Option<&str> {
        self.ralph.as_ref().map(|r| r.break_tag.as_str())
    }
}

impl Trigger {
    /// Parse a trigger string like "after-result", "after-tool:2", "after-message:1".
    pub fn parse(s: &str) -> Option<Self> {
        if s == "after-result" {
            Some(Trigger::AfterResult)
        } else if let Some(n) = s.strip_prefix("after-tool:") {
            Some(Trigger::AfterTool(n.parse().ok()?))
        } else if let Some(n) = s.strip_prefix("after-message:") {
            Some(Trigger::AfterMessage(n.parse().ok()?))
        } else {
            None
        }
    }

    /// Check if this trigger fires given current event counts.
    pub fn fires(&self, tool_count: usize, message_count: usize, got_result: bool) -> bool {
        match self {
            Trigger::AfterResult => got_result,
            Trigger::AfterTool(n) => tool_count >= *n,
            Trigger::AfterMessage(n) => message_count >= *n,
        }
    }
}
