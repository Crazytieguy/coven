use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;

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

use super::{RawModeGuard, setup_display};

/// Tag-based features gated by CLI flags.
pub struct TagFlags {
    pub fork: bool,
    pub reload: bool,
}

pub struct RalphConfig {
    pub prompt: String,
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

    fn session_config(&self, system_prompt: &str) -> SessionConfig {
        SessionConfig {
            prompt: Some(self.prompt.clone()),
            extra_args: self.extra_args.clone(),
            append_system_prompt: Some(system_prompt.to_string()),
            working_dir: self.working_dir.clone(),
            ..Default::default()
        }
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
    let _raw = RawModeGuard::acquire(vcr.is_live())?;

    let (mut renderer, mut input) = setup_display(writer, config.term_width, config.show_thinking);
    renderer.render_hints(crate::display::renderer::HintContext::Initial {
        has_wait: !config.no_wait,
    });
    let system_prompt = config.system_prompt();
    if config.tag_flags.fork {
        config.extra_args.extend(ForkConfig::disallowed_tool_args());
    }
    let fork_config = ForkConfig::if_enabled(
        config.tag_flags.fork,
        &config.extra_args,
        &config.working_dir,
    );
    let session_config = config.session_config(&system_prompt);
    let mut watched_tags = Vec::new();
    if !config.no_wait {
        watched_tags.push("wait-for-user".to_string());
    }
    if !config.no_break {
        watched_tags.push(config.break_tag.clone());
    }
    if config.tag_flags.fork {
        watched_tags.push("fork".to_string());
    }
    if config.tag_flags.reload {
        watched_tags.push("reload".to_string());
    }
    let features = SessionFeatures {
        fork_config: fork_config.as_ref(),
        reload_enabled: config.tag_flags.reload,
        base_config: &session_config,
        watched_tags,
    };

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

    'outer: loop {
        iter.iteration += 1;
        if config.iterations > 0 && iter.iteration > config.iterations {
            ctx.renderer.write_raw(&format!(
                "\r\nReached iteration limit ({})\r\n",
                config.iterations
            ));
            break;
        }
        ctx.renderer
            .write_raw(&format!("\r\n--- Iteration {} ---\r\n\r\n", iter.iteration));

        let mut runner = event_loop::spawn_session(session_config.clone(), ctx.io, ctx.vcr).await?;
        let mut state = SessionState::default();
        iter.iteration_cost = 0.0;

        loop {
            let outcome = event_loop::run_session(
                &mut runner,
                &mut state,
                ctx.renderer,
                ctx.input,
                ctx.io,
                ctx.vcr,
                &features,
            )
            .await?;
            // Kill the CLI process immediately to prevent async task
            // notifications from triggering an invisible continuation.
            runner.kill().await?;

            match handle_session_outcome(
                outcome,
                &mut state,
                &mut iter,
                &session_config,
                &config,
                &mut ctx,
            )
            .await?
            {
                LoopAction::NextIteration => break,
                LoopAction::Resume(new_runner, new_state) => {
                    runner = *new_runner;
                    state = new_state;
                }
                LoopAction::Exit => break 'outer,
            }
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
        SessionOutcome::Completed { result_text } => {
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
}
