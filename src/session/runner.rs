#![allow(dead_code)]

use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::mpsc;

use crate::event::AppEvent;
use crate::protocol::emit::format_user_message;
use crate::protocol::parse::parse_line;

/// Configuration for spawning a claude session.
pub struct SessionConfig {
    /// Initial prompt to send (if any).
    pub prompt: Option<String>,
    /// Extra arguments to pass to claude.
    pub extra_args: Vec<String>,
    /// Append to system prompt (for ralph mode).
    pub append_system_prompt: Option<String>,
}

/// Manages a claude -p subprocess with bidirectional stream-json.
pub struct SessionRunner {
    child: Child,
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
        let mut cmd = tokio::process::Command::new("claude");
        cmd.arg("-p")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .arg("--input-format")
            .arg("stream-json");

        if let Some(ref system_prompt) = config.append_system_prompt {
            cmd.arg("--append-system-prompt").arg(system_prompt);
        }

        for arg in &config.extra_args {
            cmd.arg(arg);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = cmd.spawn().context("Failed to spawn claude process")?;

        let stdout = child.stdout.take().expect("stdout should be piped");
        let mut stdin = child.stdin.take().expect("stdin should be piped");

        // Send initial prompt if provided
        if let Some(prompt) = config.prompt {
            let msg = format_user_message(&prompt);
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
            child,
            stdin: Some(stdin),
        })
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
        ];

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
        let msg = format_user_message(text);
        stdin.write_all(msg.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }

    /// Close stdin, signaling claude to finish.
    pub async fn close_input(&mut self) {
        self.stdin.take();
    }

    /// Wait for the claude process to exit.
    pub async fn wait(&mut self) -> Result<Option<i32>> {
        let status = self.child.wait().await?;
        Ok(status.code())
    }

    /// Kill the claude process.
    pub async fn kill(&mut self) -> Result<()> {
        self.child.kill().await?;
        Ok(())
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
