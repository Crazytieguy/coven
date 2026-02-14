use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::mpsc;

use crate::event::AppEvent;
use crate::protocol::emit::format_user_message;
use crate::protocol::parse::parse_line;

/// Configuration for spawning a claude session.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Initial prompt to send (if any).
    pub prompt: Option<String>,
    /// Extra arguments to pass to claude.
    pub extra_args: Vec<String>,
    /// Append to system prompt.
    pub append_system_prompt: Option<String>,
    /// Resume an existing session by ID (uses `--resume`).
    pub resume: Option<String>,
    /// Working directory for the claude process. If None, inherits from parent.
    /// Skipped in serde — not meaningful for VCR replay, only a runtime concern.
    #[serde(skip)]
    pub working_dir: Option<PathBuf>,
}

impl SessionConfig {
    /// Create a resume config by cloning this config and setting the prompt and session ID.
    #[must_use]
    pub fn resume_with(&self, prompt: String, session_id: String) -> Self {
        SessionConfig {
            prompt: Some(prompt),
            resume: Some(session_id),
            ..self.clone()
        }
    }
}

/// Manages a claude -p subprocess with bidirectional stream-json.
///
/// The `child` field is optional to support VCR replay mode, where a stub
/// `SessionRunner` is constructed without a real process.
pub struct SessionRunner {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
}

impl SessionRunner {
    /// Spawn a claude process and start reading its output.
    ///
    /// Parsed events are sent to `event_tx`. The initial prompt (if any)
    /// is sent as the first stdin message.
    pub async fn spawn(
        config: SessionConfig,
        event_tx: mpsc::UnboundedSender<AppEvent>,
    ) -> Result<Self> {
        let args = Self::build_args(&config);
        let mut cmd = tokio::process::Command::new("claude");
        cmd.args(&args);

        if let Some(ref dir) = config.working_dir {
            cmd.current_dir(dir);
        }

        // Coven launches independent `-p` mode sessions, not nested interactive
        // ones. Remove CLAUDECODE so the CLI doesn't reject the invocation.
        cmd.env_remove("CLAUDECODE");

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = cmd.spawn().context("Failed to spawn claude process")?;

        let stdout = child.stdout.take().context("stdout should be piped")?;
        let mut stdin = child.stdin.take().context("stdin should be piped")?;

        // Send initial prompt if provided
        if let Some(prompt) = config.prompt {
            let msg = format_user_message(&prompt)?;
            stdin
                .write_all(msg.as_bytes())
                .await
                .context("Failed to write initial prompt to claude stdin")?;
            stdin
                .write_all(b"\n")
                .await
                .context("Failed to write newline after initial prompt")?;
            stdin.flush().await?;
        }

        // Spawn stdout reader task
        Self::spawn_reader(stdout, event_tx);

        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
        })
    }

    /// Create a stub `SessionRunner` for VCR replay mode.
    /// Has no real process — methods like `close_input`/`wait`/`kill` are no-ops.
    pub fn stub() -> Self {
        Self {
            child: None,
            stdin: None,
        }
    }

    /// Build the claude CLI arguments (for VCR header validation).
    pub fn build_args(config: &SessionConfig) -> Vec<String> {
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--input-format".to_string(),
            "stream-json".to_string(),
            "--include-partial-messages".to_string(),
        ];

        if let Some(ref session_id) = config.resume {
            args.push("--resume".to_string());
            args.push(session_id.clone());
        }

        if !has_flag(&config.extra_args, "--permission-mode") {
            args.push("--permission-mode".to_string());
            args.push("acceptEdits".to_string());
        }

        if !has_flag(&config.extra_args, "--max-thinking-tokens") {
            args.push("--max-thinking-tokens".to_string());
            args.push("31999".to_string());
        }

        if let Some(ref system_prompt) = config.append_system_prompt {
            args.push("--append-system-prompt".to_string());
            args.push(system_prompt.clone());
        }

        args.extend(config.extra_args.iter().cloned());
        args
    }

    /// Send a user message to claude's stdin.
    pub async fn send_message(&mut self, text: &str) -> Result<()> {
        let stdin = self.stdin.as_mut().context("stdin already closed")?;
        let msg = format_user_message(text)?;
        stdin.write_all(msg.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    /// Close stdin, signaling claude to finish.
    pub fn close_input(&mut self) {
        self.stdin.take();
    }

    /// Wait for the claude process to exit. No-op on stubs.
    pub async fn wait(&mut self) -> Result<Option<i32>> {
        match &mut self.child {
            Some(child) => Ok(child.wait().await?.code()),
            None => Ok(None),
        }
    }

    /// Kill the claude process. No-op on stubs.
    pub async fn kill(&mut self) -> Result<()> {
        if let Some(child) = &mut self.child {
            child.kill().await?;
        }
        Ok(())
    }

    /// Scan response text for `<tag>reason</tag>` and return the reason if found.
    /// Note: the raw break tag is intentionally left visible in Claude's streamed output
    /// so the user can see exactly what Claude emitted. The clean "Loop complete: reason"
    /// line is added separately after the stats display (see ralph.rs).
    pub fn scan_break_tag(text: &str, tag: &str) -> Option<String> {
        crate::protocol::parse::extract_tag_inner(text, tag).map(|s| s.trim().to_string())
    }

    /// Build the ralph system prompt for the given break tag.
    pub fn ralph_system_prompt(break_tag: &str) -> String {
        format!(
            "You are running in a multi-iteration loop. Each iteration starts a fresh session \
             but the filesystem persists. The loop is designed to run many iterations — each \
             one you do a small piece of work, then end your response normally. The next \
             iteration starts automatically.\n\n\
             Only include `<{break_tag}>reason</{break_tag}>` to end the entire loop. This is \
             rare — only do it when you have exhausted all available work and another iteration \
             would accomplish nothing new."
        )
    }

    fn spawn_reader(stdout: ChildStdout, event_tx: mpsc::UnboundedSender<AppEvent>) {
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                match parse_line(&line) {
                    Ok(Some(event)) => {
                        if event_tx.send(AppEvent::Claude(Box::new(event))).is_err() {
                            break;
                        }
                    }
                    Ok(None) => {} // empty line
                    Err(e) => {
                        let warning = format!("Failed to parse claude output: {e}\n  Line: {line}");
                        if event_tx.send(AppEvent::ParseWarning(warning)).is_err() {
                            break;
                        }
                    }
                }
            }

            // stdout closed — process is exiting or has exited
            let _ = event_tx.send(AppEvent::ProcessExit(None));
        });
    }
}

/// Check whether a flag is already present in the extra args.
pub(crate) fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter()
        .any(|a| a == flag || a.starts_with(&format!("{flag}=")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        assert!(has_flag(
            &["--permission-mode".into(), "plan".into()],
            "--permission-mode"
        ));
    }

    #[test]
    fn equals_syntax() {
        assert!(has_flag(
            &["--permission-mode=plan".into()],
            "--permission-mode"
        ));
    }

    #[test]
    fn not_present() {
        assert!(!has_flag(
            &["--model".into(), "opus".into()],
            "--permission-mode"
        ));
    }

    #[test]
    fn empty_args() {
        assert!(!has_flag(&[], "--permission-mode"));
    }

    #[test]
    fn prefix_not_false_positive() {
        assert!(!has_flag(
            &["--permission-mode-extra".into()],
            "--permission-mode"
        ));
    }
}
