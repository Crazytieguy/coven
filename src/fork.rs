use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::display::renderer::Renderer;
use crate::event::AppEvent;
use crate::protocol::types::{AssistantContentBlock, InboundEvent};
use crate::session::runner::{SessionConfig, SessionRunner};
use crate::vcr::VcrContext;

/// Configuration for fork behavior, threaded through the session loop.
pub struct ForkConfig {
    pub extra_args: Vec<String>,
    pub working_dir: Option<PathBuf>,
}

impl ForkConfig {
    /// Build a `ForkConfig` if forking is enabled, otherwise `None`.
    pub fn if_enabled(
        enabled: bool,
        extra_args: &[String],
        working_dir: &Option<PathBuf>,
    ) -> Option<Self> {
        enabled.then(|| Self {
            extra_args: extra_args.to_vec(),
            working_dir: working_dir.clone(),
        })
    }
}

/// Run the fork flow: spawn children in parallel, render output, collect results.
///
/// Each child resumes from the parent session with `--fork-session` and receives
/// a simple follow-up prompt identifying its assigned task. Child events are
/// multiplexed onto a shared channel and rendered with fork-specific styling.
///
/// Returns the XML reintegration message to send back to the parent session.
pub async fn run_fork<W: Write>(
    parent_session_id: &str,
    tasks: Vec<String>,
    config: &ForkConfig,
    renderer: &mut Renderer<W>,
    vcr: &VcrContext,
) -> Result<String> {
    renderer.render_fork_start(&tasks);

    let num_tasks = tasks.len();
    let (merged_tx, mut merged_rx) = mpsc::unbounded_channel::<(usize, AppEvent)>();
    let mut runners: Vec<SessionRunner> = Vec::new();

    for (i, task) in tasks.iter().enumerate() {
        let (child_tx, mut child_rx) = mpsc::unbounded_channel();
        let mut extra_args = config.extra_args.clone();
        extra_args.push("--fork-session".to_string());
        let child_config = SessionConfig {
            prompt: Some(format!("You were assigned '{task}'")),
            resume: Some(parent_session_id.to_string()),
            extra_args,
            working_dir: config.working_dir.clone(),
            ..Default::default()
        };

        // In replay mode, the closure is never called: child_tx is dropped,
        // child_rx.recv() returns None, and the multiplexer task exits cleanly.
        let runner = vcr
            .call("fork_spawn", child_config, async |c: &SessionConfig| {
                SessionRunner::spawn(c.clone(), child_tx).await
            })
            .await?;
        runners.push(runner);

        let merged_tx = merged_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = child_rx.recv().await {
                if merged_tx.send((i, event)).is_err() {
                    break;
                }
            }
        });
    }
    drop(merged_tx);

    // Process events from all children. Each event is individually recorded
    // so fork child tool calls and completions appear in VCR test snapshots.
    let mut results: Vec<Option<std::result::Result<String, String>>> = vec![None; num_tasks];
    let mut completed = 0;

    loop {
        let event: Option<(usize, AppEvent)> = vcr
            .call("fork_event", (), async |(): &()| Ok(merged_rx.recv().await))
            .await?;
        let Some((idx, event)) = event else { break };

        match event {
            AppEvent::Claude(inbound) => match &*inbound {
                InboundEvent::Assistant(msg) if msg.parent_tool_use_id.is_none() => {
                    for block in &msg.message.content {
                        if let AssistantContentBlock::ToolUse { name, input, .. } = block {
                            renderer.render_fork_child_tool_call(name, input);
                        }
                    }
                }
                InboundEvent::Result(result) => {
                    renderer.render_fork_child_done(&tasks[idx]);
                    results[idx] = Some(Ok(result.result.clone()));
                    completed += 1;
                    if completed == num_tasks {
                        break;
                    }
                }
                _ => {}
            },
            AppEvent::ParseWarning(w) => {
                renderer.render_warning(&w);
            }
            AppEvent::ProcessExit(_) => {
                if results[idx].is_none() {
                    results[idx] = Some(Err("Child process exited unexpectedly".to_string()));
                    completed += 1;
                    if completed == num_tasks {
                        break;
                    }
                }
            }
        }
    }

    for runner in &mut runners {
        runner.close_input();
        let _ = runner.wait().await;
    }

    renderer.render_fork_complete();

    let result_tuples: Vec<(String, std::result::Result<String, String>)> = tasks
        .into_iter()
        .zip(results)
        .map(|(label, result)| {
            let outcome = result.unwrap_or_else(|| Err("No result received".to_string()));
            (label, outcome)
        })
        .collect();

    Ok(compose_reintegration_message(&result_tuples))
}

/// Parse a `<fork>` tag from response text and return the task labels.
///
/// The tag contains a YAML-style list of task labels:
/// ```text
/// <fork>
/// - Refactor auth module
/// - Add tests for user API
/// </fork>
/// ```
pub fn parse_fork_tag(text: &str) -> Option<Vec<String>> {
    let inner = crate::protocol::parse::extract_tag_inner(text, "fork")?;

    let tasks: Vec<String> = inner
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.strip_prefix("- ").unwrap_or(line).trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if tasks.is_empty() { None } else { Some(tasks) }
}

/// Compose the XML reintegration message sent back to the parent session.
///
/// Each task's result (or error) is wrapped in a `<task>` element inside
/// `<fork-results>`, so the parent model can see what each child produced.
pub fn compose_reintegration_message(results: &[(String, Result<String, String>)]) -> String {
    use std::fmt::Write;

    let mut xml = String::from("<fork-results>\n");
    for (label, outcome) in results {
        // Escape label for XML attribute
        let safe_label = label
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;");
        match outcome {
            Ok(text) => {
                let _ = write!(
                    xml,
                    "<task label=\"{safe_label}\">\n<![CDATA[{text}]]>\n</task>\n"
                );
            }
            Err(err) => {
                let _ = write!(
                    xml,
                    "<task label=\"{safe_label}\" error=\"true\">\n<![CDATA[{err}]]>\n</task>\n"
                );
            }
        }
    }
    xml.push_str("</fork-results>");
    xml
}

/// Build the system prompt fragment that teaches the model about forking.
pub fn fork_system_prompt() -> &'static str {
    "To parallelize work, emit a <fork> tag containing a YAML list of short task labels:\n\
     <fork>\n\
     - Refactor auth module\n\
     - Add tests for user API\n\
     </fork>\n\
     Each fork inherits your full context and runs in parallel. You'll receive the results \
     in a <fork-results> message when all children complete."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fork_tag_basic() {
        let text = "Let me split this up.\n<fork>\n- Refactor auth\n- Add tests\n</fork>\nDone.";
        assert_eq!(
            parse_fork_tag(text),
            Some(vec!["Refactor auth".to_string(), "Add tests".to_string()])
        );
    }

    #[test]
    fn parse_fork_tag_single_task() {
        let text = "<fork>\n- Just one thing\n</fork>";
        assert_eq!(
            parse_fork_tag(text),
            Some(vec!["Just one thing".to_string()])
        );
    }

    #[test]
    fn parse_fork_tag_no_tag() {
        assert_eq!(parse_fork_tag("no fork here"), None);
    }

    #[test]
    fn parse_fork_tag_empty_list() {
        let text = "<fork>\n\n</fork>";
        assert_eq!(parse_fork_tag(text), None);
    }

    #[test]
    fn parse_fork_tag_partial() {
        let text = "<fork>\n- item\n but no closing tag";
        assert_eq!(parse_fork_tag(text), None);
    }

    #[test]
    fn parse_fork_tag_extra_whitespace() {
        let text = "<fork>\n  - spaced out  \n  - another  \n</fork>";
        assert_eq!(
            parse_fork_tag(text),
            Some(vec!["spaced out".to_string(), "another".to_string()])
        );
    }

    #[test]
    fn compose_reintegration_message_success() {
        let results = vec![
            ("Task A".to_string(), Ok("Result A".to_string())),
            ("Task B".to_string(), Ok("Result B".to_string())),
        ];
        let msg = compose_reintegration_message(&results);
        assert!(msg.starts_with("<fork-results>"));
        assert!(msg.ends_with("</fork-results>"));
        assert!(msg.contains("<task label=\"Task A\">"));
        assert!(msg.contains("<![CDATA[Result A]]>"));
        assert!(msg.contains("<task label=\"Task B\">"));
        assert!(msg.contains("<![CDATA[Result B]]>"));
    }

    #[test]
    fn compose_reintegration_message_with_error() {
        let results = vec![
            ("Good".to_string(), Ok("worked".to_string())),
            ("Bad".to_string(), Err("process crashed".to_string())),
        ];
        let msg = compose_reintegration_message(&results);
        assert!(msg.contains("<task label=\"Good\">"));
        assert!(msg.contains("<task label=\"Bad\" error=\"true\">"));
        assert!(msg.contains("<![CDATA[process crashed]]>"));
    }

    #[test]
    fn compose_reintegration_message_handles_angle_brackets() {
        let results = vec![(
            "Fix code".to_string(),
            Ok("Changed Vec<String> to Vec<&str>".to_string()),
        )];
        let msg = compose_reintegration_message(&results);
        assert!(msg.contains("<![CDATA[Changed Vec<String> to Vec<&str>]]>"));
    }

    #[test]
    fn compose_reintegration_message_escapes_label() {
        let results = vec![("Fix \"quotes\"".to_string(), Ok("done".to_string()))];
        let msg = compose_reintegration_message(&results);
        assert!(msg.contains("label=\"Fix &quot;quotes&quot;\""));
    }

    #[test]
    fn compose_reintegration_message_escapes_ampersand_and_angle() {
        let results = vec![("Fix A & B".to_string(), Ok("done".to_string()))];
        let msg = compose_reintegration_message(&results);
        assert!(msg.contains("label=\"Fix A &amp; B\""));

        let results = vec![("Fix <thing>".to_string(), Ok("done".to_string()))];
        let msg = compose_reintegration_message(&results);
        assert!(msg.contains("label=\"Fix &lt;thing&gt;\""));
    }

    #[test]
    fn fork_system_prompt_contains_tag() {
        let prompt = fork_system_prompt();
        assert!(prompt.contains("<fork>"));
        assert!(prompt.contains("</fork>"));
        assert!(prompt.contains("<fork-results>"));
    }
}
