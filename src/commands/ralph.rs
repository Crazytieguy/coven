use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::display::input::InputHandler;
use crate::display::renderer::{Renderer, StoredMessage};
use crate::fork::{self, ForkConfig};
use crate::protocol::parse::extract_tag_inner;
use crate::reload;
use crate::session::runner::{SessionConfig, SessionRunner};
use crate::session::state::SessionState;
use crate::vcr::{Io, VcrContext};

use crate::session::event_loop::{self, SessionFeatures, SessionOutcome};
use crate::transition::WAIT_FOR_USER_PROMPT;

use super::{RawModeGuard, render_initial_hints, setup_display};

/// Tag-based features gated by CLI flags.
pub struct TagFlags {
    pub fork: bool,
    pub reload: bool,
}

/// Where each iteration's prompt comes from.
pub enum PromptSource {
    /// Same prompt every iteration.
    Static(String),
    /// Run this shell command (`sh -c <CMD>`) with `COVEN_ITERATION` set;
    /// its stdout is the prompt. Non-zero exit or empty stdout ends the loop.
    Command(String),
}

impl PromptSource {
    /// Build a `PromptSource` from the two mutually-exclusive CLI/fixture
    /// fields. `prompt_command` takes precedence when both are present.
    pub fn from_cli(prompt: Option<String>, prompt_command: Option<String>) -> Result<Self> {
        match (prompt, prompt_command) {
            (_, Some(cmd)) => Ok(PromptSource::Command(cmd)),
            (Some(p), None) => Ok(PromptSource::Static(p)),
            (None, None) => {
                anyhow::bail!("ralph needs a positional prompt or --prompt-command")
            }
        }
    }
}

pub struct RalphConfig {
    pub prompt_source: PromptSource,
    pub iterations: u32,
    pub break_tag: String,
    pub no_break: bool,
    pub no_wait: bool,
    pub show_thinking: bool,
    pub tag_flags: TagFlags,
    pub extra_args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    /// Override terminal width for display truncation (used in tests).
    pub term_width: Option<usize>,
}

impl RalphConfig {
    fn system_prompt(&self) -> String {
        let base = if self.no_break {
            "After you respond, a new session will start with the same prompt and the \
             filesystem as you left it. This repeats automatically."
                .to_string()
        } else {
            ralph_system_prompt(&self.break_tag, self.no_wait)
        };
        let mut prompt = if self.tag_flags.fork {
            format!("{base}\n\n{}", fork::fork_system_prompt())
        } else {
            base
        };
        if self.tag_flags.reload {
            prompt = format!("{prompt}\n\n{}", reload::reload_system_prompt());
        }
        prompt
    }

    fn session_config(&self, system_prompt: &str, prompt: String) -> SessionConfig {
        SessionConfig {
            prompt: Some(prompt),
            extra_args: self.extra_args.clone(),
            append_system_prompt: Some(system_prompt.to_string()),
            working_dir: self.working_dir.clone(),
            ..Default::default()
        }
    }

    fn watched_tags(&self) -> Vec<String> {
        let mut tags = Vec::new();
        if !self.no_wait {
            tags.push("wait-for-user".to_string());
        }
        if !self.no_break {
            tags.push(self.break_tag.clone());
        }
        if self.tag_flags.fork {
            tags.push("fork".to_string());
        }
        if self.tag_flags.reload {
            tags.push("reload".to_string());
        }
        tags
    }

    /// Check for `<break>` tag (respects `--no-break`).
    fn scan_break(&self, text: &str) -> Option<String> {
        if self.no_break {
            None
        } else {
            scan_break_tag(text, &self.break_tag)
        }
    }
}

/// Scan response text for `<tag>reason</tag>` and return the reason if found.
fn scan_break_tag(text: &str, tag: &str) -> Option<String> {
    extract_tag_inner(text, tag).map(|s| s.trim().to_string())
}

/// Recorded output of running the prompt command once.
#[derive(Debug, Serialize, Deserialize)]
struct PromptCommandOutput {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

/// Arguments recorded for a prompt command invocation. `working_dir` is
/// deliberately excluded so the recorded tuple doesn't diverge between
/// record (which passes `Some(tmp_dir)`) and replay (which may not).
#[derive(Debug, Serialize, Deserialize)]
struct PromptCommandArgs {
    command: String,
    iteration: u32,
}

/// Result of asking the prompt source for the next iteration's prompt.
enum PromptResolution {
    /// Use this prompt for the next iteration.
    Prompt(String),
    /// Stop the loop; `reason` is shown to the user.
    Exhausted(String),
}

/// Resolve the prompt for the current iteration.
async fn resolve_prompt(
    source: &PromptSource,
    iteration: u32,
    working_dir: Option<&Path>,
    vcr: &VcrContext,
) -> Result<PromptResolution> {
    match source {
        PromptSource::Static(s) => Ok(PromptResolution::Prompt(s.clone())),
        PromptSource::Command(cmd) => {
            let args = PromptCommandArgs {
                command: cmd.clone(),
                iteration,
            };
            let output = vcr
                .call("prompt_command", args, async |a: &PromptCommandArgs| {
                    run_prompt_command(&a.command, a.iteration, working_dir).await
                })
                .await?;
            Ok(interpret_prompt_command(&output))
        }
    }
}

async fn run_prompt_command(
    command: &str,
    iteration: u32,
    working_dir: Option<&Path>,
) -> Result<PromptCommandOutput> {
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c")
        .arg(command)
        .env("COVEN_ITERATION", iteration.to_string());
    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }
    let output = cmd
        .output()
        .await
        .with_context(|| format!("failed to spawn prompt command: {command}"))?;
    Ok(PromptCommandOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

fn interpret_prompt_command(output: &PromptCommandOutput) -> PromptResolution {
    if output.exit_code != 0 {
        let stderr = output.stderr.trim();
        let reason = if stderr.is_empty() {
            format!("prompt command exited with status {}", output.exit_code)
        } else {
            format!(
                "prompt command exited with status {}: {stderr}",
                output.exit_code
            )
        };
        return PromptResolution::Exhausted(reason);
    }
    let trimmed = output.stdout.trim();
    if trimmed.is_empty() {
        PromptResolution::Exhausted("prompt command produced no output".to_string())
    } else {
        PromptResolution::Prompt(trimmed.to_string())
    }
}

/// Build the ralph system prompt for the given break tag.
fn ralph_system_prompt(break_tag: &str, no_wait: bool) -> String {
    let mut prompt = format!(
        "After you respond, a new session will start with the same prompt and the filesystem \
         as you left it. This repeats automatically — no special action is needed to continue.\n\n\
         `<{break_tag}>reason</{break_tag}>` permanently ends the loop. Before using it, \
         consider: if a new session received the same prompt and looked at the current state of \
         the project, would it find something worth doing? If yes, don't break — finishing your \
         current work doesn't mean the next session has nothing to do. If no, use `<{break_tag}>` \
         to stop. When in doubt, let the loop continue."
    );
    if !no_wait {
        prompt.push_str("\n\n");
        prompt.push_str(WAIT_FOR_USER_PROMPT);
    }
    prompt
}

/// Mutable I/O handles shared across the ralph loop.
struct Ctx<'a, W: Write> {
    input: &'a mut InputHandler,
    renderer: &'a mut Renderer<W>,
    io: &'a mut Io,
    vcr: &'a VcrContext,
}

/// Per-loop cost and iteration tracking.
struct IterState {
    iteration: u32,
    iteration_cost: f64,
    total_cost: f64,
}

/// Run ralph loop mode.
pub async fn ralph<W: Write>(
    mut config: RalphConfig,
    io: &mut Io,
    vcr: &VcrContext,
    writer: W,
) -> Result<Vec<StoredMessage>> {
    // Headless: force-disable wait-for-user. There's no human to respond,
    // so the model must not be told the feature exists, and any tag it
    // emits anyway must be ignored rather than "dismissed" silently.
    if io.is_headless() {
        config.no_wait = true;
    }
    let _raw = RawModeGuard::acquire(io)?;

    let (mut renderer, mut input) = setup_display(writer, config.term_width, config.show_thinking);
    render_initial_hints(&mut renderer, io, !config.no_wait);
    let system_prompt = config.system_prompt();
    if config.tag_flags.fork {
        config.extra_args.extend(ForkConfig::disallowed_tool_args());
    }
    let fork_config = ForkConfig::if_enabled(
        config.tag_flags.fork,
        &config.extra_args,
        &config.working_dir,
    );
    let watched_tags = config.watched_tags();

    let mut ctx = Ctx {
        input: &mut input,
        renderer: &mut renderer,
        io,
        vcr,
    };
    let mut iter = IterState {
        iteration: 0,
        iteration_cost: 0.0,
        total_cost: 0.0,
    };

    loop {
        iter.iteration += 1;
        if config.iterations > 0 && iter.iteration > config.iterations {
            ctx.renderer.write_raw(&format!(
                "\r\nReached iteration limit ({})\r\n",
                config.iterations
            ));
            break;
        }

        let prompt = match resolve_prompt(
            &config.prompt_source,
            iter.iteration,
            config.working_dir.as_deref(),
            ctx.vcr,
        )
        .await?
        {
            PromptResolution::Prompt(p) => p,
            PromptResolution::Exhausted(reason) => {
                ctx.renderer
                    .write_raw(&format!("\r\nPrompt source exhausted: {reason}\r\n"));
                break;
            }
        };

        ctx.renderer
            .write_raw(&format!("\r\n--- Iteration {} ---\r\n\r\n", iter.iteration));

        let session_config = config.session_config(&system_prompt, prompt);
        let features = SessionFeatures {
            fork_config: fork_config.as_ref(),
            reload_enabled: config.tag_flags.reload,
            base_config: &session_config,
            watched_tags: watched_tags.clone(),
        };

        iter.iteration_cost = 0.0;
        match run_iteration(&session_config, &features, &config, &mut iter, &mut ctx).await? {
            IterationResult::Next => continue,
            IterationResult::Exit => break,
        }
    }

    Ok(renderer.into_messages())
}

/// What to do after handling a session outcome.
enum LoopAction {
    /// Start the next iteration of the ralph loop.
    NextIteration,
    /// Continue the inner session loop (session was resumed).
    Resume(Box<SessionRunner>, SessionState),
    /// Exit the ralph loop entirely.
    Exit,
}

enum IterationResult {
    Next,
    Exit,
}

async fn run_iteration<W: Write>(
    session_config: &SessionConfig,
    features: &SessionFeatures<'_>,
    config: &RalphConfig,
    iter: &mut IterState,
    ctx: &mut Ctx<'_, W>,
) -> Result<IterationResult> {
    let mut runner = event_loop::spawn_session(session_config.clone(), ctx.io, ctx.vcr).await?;
    let mut state = SessionState::default();

    loop {
        let outcome = event_loop::run_session(
            &mut runner,
            &mut state,
            ctx.renderer,
            ctx.input,
            ctx.io,
            ctx.vcr,
            features,
        )
        .await?;
        // Wait for session file persistence before killing, so the session
        // can be safely resumed. Skip for interrupts/exits.
        if matches!(
            outcome,
            SessionOutcome::Completed { .. } | SessionOutcome::Reload { .. }
        ) {
            crate::session::persist::wait_if_needed(&state, ctx.vcr, config.working_dir.as_deref())
                .await;
        }
        runner.kill().await?;

        match handle_session_outcome(outcome, &mut state, iter, session_config, config, ctx).await?
        {
            LoopAction::NextIteration => return Ok(IterationResult::Next),
            LoopAction::Resume(new_runner, new_state) => {
                runner = *new_runner;
                state = new_state;
            }
            LoopAction::Exit => return Ok(IterationResult::Exit),
        }
    }
}

/// Process a session outcome: handle completion (wait-for-user, break tag),
/// interrupts, and process exits.
async fn handle_session_outcome<W: Write>(
    outcome: SessionOutcome,
    state: &mut SessionState,
    iter: &mut IterState,
    session_config: &SessionConfig,
    config: &RalphConfig,
    ctx: &mut Ctx<'_, W>,
) -> Result<LoopAction> {
    match outcome {
        SessionOutcome::Completed { result_text, .. } => {
            iter.iteration_cost += state.total_cost_usd;
            iter.total_cost += iter.iteration_cost;
            ctx.renderer
                .write_raw(&format!("  Total cost: ${:.2}\r\n", iter.total_cost));

            // User pressed Ctrl+W — wait for input before continuing.
            // Escape dismisses the wait and falls through to tag processing.
            if state.wait_requested {
                state.wait_requested = false;
                ctx.renderer.write_raw("\x07");
                ctx.renderer.write_raw("\r\n[waiting for user input]\r\n");
                match wait_input_and_resume(state, session_config, ctx).await? {
                    WaitResumeAction::Resume(runner, new_state) => {
                        iter.iteration_cost = 0.0;
                        return Ok(LoopAction::Resume(runner, new_state));
                    }
                    WaitResumeAction::Dismissed => {
                        // Fall through to tag processing below.
                    }
                    WaitResumeAction::Exit => return Ok(LoopAction::Exit),
                }
            }

            // Check for wait-for-user before break tag (user input takes precedence).
            if !config.no_wait
                && let Some(reason) =
                    crate::protocol::parse::extract_tag_inner(&result_text, "wait-for-user")
            {
                let reason = reason.trim();
                ctx.renderer.write_raw("\x07");
                ctx.renderer
                    .write_raw(&format!("\r\nWaiting for user: {reason}\r\n"));
                match wait_input_and_resume(state, session_config, ctx).await? {
                    WaitResumeAction::Resume(runner, new_state) => {
                        iter.iteration_cost = 0.0;
                        return Ok(LoopAction::Resume(runner, new_state));
                    }
                    WaitResumeAction::Dismissed => {
                        // Fall through to break tag check below.
                    }
                    WaitResumeAction::Exit => return Ok(LoopAction::Exit),
                }
            }

            if let Some(reason) = config.scan_break(&result_text) {
                let s = if iter.iteration == 1 { "" } else { "s" };
                ctx.renderer.write_raw(&format!(
                    "\r\nLoop complete ({} iteration{s}): {reason}\r\n",
                    iter.iteration
                ));
                return Ok(LoopAction::Exit);
            }

            Ok(LoopAction::NextIteration)
        }
        SessionOutcome::Interrupted => {
            state.wait_requested = false;
            iter.iteration_cost += state.total_cost_usd;
            ctx.renderer.render_interrupted();
            match wait_input_and_resume(state, session_config, ctx).await? {
                WaitResumeAction::Resume(runner, new_state) => {
                    Ok(LoopAction::Resume(runner, new_state))
                }
                WaitResumeAction::Dismissed | WaitResumeAction::Exit => Ok(LoopAction::Exit),
            }
        }
        SessionOutcome::Reload { .. } => {
            let Some(session_id) = state.session_id.take() else {
                return Ok(LoopAction::Exit);
            };
            let (runner, new_state) = reload::spawn_reload_session(
                session_id,
                session_config,
                ctx.renderer,
                ctx.io,
                ctx.vcr,
            )
            .await?;
            iter.iteration_cost = 0.0;
            Ok(LoopAction::Resume(Box::new(runner), new_state))
        }
        SessionOutcome::ProcessExited => Ok(LoopAction::Exit),
    }
}

/// What to do after waiting for user input at a pause point.
enum WaitResumeAction {
    /// User provided text — resume with a new session.
    Resume(Box<SessionRunner>, SessionState),
    /// User dismissed the wait (Escape) — proceed without resuming.
    Dismissed,
    /// User exited (Ctrl+C / Ctrl+D).
    Exit,
}

/// Wait for user input and spawn a resumed session.
///
/// Takes the `session_id` from `state`. On `Dismissed`, the session ID is
/// restored so the caller can fall through to further processing.
async fn wait_input_and_resume<W: Write>(
    state: &mut SessionState,
    session_config: &SessionConfig,
    ctx: &mut Ctx<'_, W>,
) -> Result<WaitResumeAction> {
    let Some(session_id) = state.session_id.take() else {
        return Ok(WaitResumeAction::Exit);
    };
    match event_loop::wait_for_dismissable_input(
        ctx.input,
        ctx.renderer,
        ctx.io,
        ctx.vcr,
        &session_id,
        session_config,
    )
    .await?
    {
        Some(event_loop::WaitInterruptResult::Text(text)) => {
            let resume_config = session_config.resume_with(text, session_id.clone());
            let runner = event_loop::spawn_session(resume_config, ctx.io, ctx.vcr).await?;
            let new_state = SessionState {
                session_id: Some(session_id),
                ..Default::default()
            };
            Ok(WaitResumeAction::Resume(Box::new(runner), new_state))
        }
        Some(event_loop::WaitInterruptResult::Dismissed) => {
            state.session_id = Some(session_id);
            Ok(WaitResumeAction::Dismissed)
        }
        None => Ok(WaitResumeAction::Exit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_break_tag_found() {
        let text = "I've completed the task. <break>All bugs are fixed.</break> Done.";
        assert_eq!(
            scan_break_tag(text, "break"),
            Some("All bugs are fixed.".to_string())
        );
    }

    #[test]
    fn scan_break_tag_custom() {
        let text = "Done! <done>Everything works</done>";
        assert_eq!(
            scan_break_tag(text, "done"),
            Some("Everything works".to_string())
        );
    }

    #[test]
    fn scan_break_tag_not_found() {
        let text = "Still working on the bugs.";
        assert_eq!(scan_break_tag(text, "break"), None);
    }

    #[test]
    fn scan_break_tag_partial() {
        let text = "Found <break> but no closing tag";
        assert_eq!(scan_break_tag(text, "break"), None);
    }

    fn out(stdout: &str, stderr: &str, exit_code: i32) -> PromptCommandOutput {
        PromptCommandOutput {
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            exit_code,
        }
    }

    #[test]
    fn interpret_success_trims_stdout() {
        match interpret_prompt_command(&out("  hello world  \n", "", 0)) {
            PromptResolution::Prompt(p) => assert_eq!(p, "hello world"),
            PromptResolution::Exhausted(_) => panic!("expected Prompt"),
        }
    }

    #[test]
    fn interpret_empty_stdout_is_exhausted() {
        match interpret_prompt_command(&out("   \n\n", "", 0)) {
            PromptResolution::Exhausted(reason) => {
                assert!(reason.contains("no output"), "got: {reason}");
            }
            PromptResolution::Prompt(_) => panic!("expected Exhausted"),
        }
    }

    #[test]
    fn interpret_nonzero_exit_is_exhausted_with_stderr() {
        match interpret_prompt_command(&out("", "boom\n", 1)) {
            PromptResolution::Exhausted(reason) => {
                assert!(reason.contains("status 1"), "got: {reason}");
                assert!(reason.contains("boom"), "got: {reason}");
            }
            PromptResolution::Prompt(_) => panic!("expected Exhausted"),
        }
    }

    #[test]
    fn interpret_nonzero_exit_without_stderr() {
        match interpret_prompt_command(&out("", "", 2)) {
            PromptResolution::Exhausted(reason) => {
                assert_eq!(reason, "prompt command exited with status 2");
            }
            PromptResolution::Prompt(_) => panic!("expected Exhausted"),
        }
    }
}
