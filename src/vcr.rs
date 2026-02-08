use std::collections::HashMap;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::session::runner::{SessionConfig, SessionRunner};

/// Test case definition loaded from a `.toml` file.
#[derive(Deserialize, Default)]
pub struct TestCase {
    /// Configuration for a standard run.
    pub run: Option<RunConfig>,
    /// Configuration for ralph loop mode.
    pub ralph: Option<RalphConfig>,
    /// Display/renderer configuration for test replay.
    #[serde(default)]
    pub display: DisplayConfig,
    /// Files to create in the working directory before recording.
    #[serde(default)]
    pub files: HashMap<String, String>,
    /// Additional messages to send during the session (follow-ups, steering).
    #[serde(default)]
    pub messages: Vec<TestMessage>,
}

/// Display configuration for test replay (not used during recording).
#[derive(Deserialize, Default)]
pub struct DisplayConfig {
    /// Whether to stream thinking text inline.
    #[serde(default)]
    pub show_thinking: bool,
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
    #[serde(rename = "_vcr")]
    pub vcr: String,
    pub command: Vec<String>,
}

/// Trigger types parsed from TestMessage.trigger strings.
pub enum Trigger {
    AfterResult,
    AfterTool(usize),
    AfterMessage(usize),
}

impl TestCase {
    /// Build a `SessionConfig` from this test case.
    ///
    /// # Errors
    ///
    /// Returns an error if the test case has neither `[run]` nor `[ralph]` section.
    pub fn session_config(&self) -> Result<SessionConfig> {
        if let Some(ref ralph) = self.ralph {
            Ok(SessionConfig {
                prompt: Some(ralph.prompt.clone()),
                extra_args: ralph.claude_args.clone(),
                append_system_prompt: Some(SessionRunner::ralph_system_prompt(&ralph.break_tag)),
                resume: None,
            })
        } else if let Some(ref run) = self.run {
            Ok(SessionConfig {
                prompt: Some(run.prompt.clone()),
                extra_args: run.claude_args.clone(),
                append_system_prompt: None,
                resume: None,
            })
        } else {
            bail!("Test case must have either [run] or [ralph] section");
        }
    }

    /// Build the expected CLI command (including "claude" prefix).
    ///
    /// # Errors
    ///
    /// Returns an error if the test case has neither `[run]` nor `[ralph]` section.
    pub fn expected_command(&self) -> Result<Vec<String>> {
        let config = self.session_config()?;
        let mut cmd = vec!["claude".to_string()];
        cmd.extend(SessionRunner::build_args(&config));
        Ok(cmd)
    }

    /// Get the initial prompt.
    ///
    /// # Errors
    ///
    /// Returns an error if the test case has neither `[run]` nor `[ralph]` section.
    pub fn prompt(&self) -> Result<&str> {
        if let Some(ref ralph) = self.ralph {
            Ok(&ralph.prompt)
        } else if let Some(ref run) = self.run {
            Ok(&run.prompt)
        } else {
            bail!("Test case must have either [run] or [ralph] section");
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
