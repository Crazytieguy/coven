use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;

use crate::display::input::InputHandler;
use crate::display::renderer::{Renderer, StoredMessage};
use crate::fork::{self, ForkConfig};
use crate::reload;
use crate::session::runner::{SessionConfig, SessionRunner};
use crate::session::state::{SessionState, SessionStatus};
use crate::vcr::{Io, VcrContext};

use crate::session::event_loop::{self, FollowUpAction, SessionFeatures, SessionOutcome};

use super::{RawModeGuard, setup_display};

pub struct RunConfig {
    pub prompt: Option<String>,
    pub extra_args: Vec<String>,
    pub show_thinking: bool,
    pub fork: bool,
    pub reload: bool,
    pub working_dir: Option<PathBuf>,
    /// Override terminal width for display truncation (used in tests).
    pub term_width: Option<usize>,
}

struct Ctx<'a, W: Write> {
    input: &'a mut InputHandler,
    renderer: &'a mut Renderer<W>,
    io: &'a mut Io,
    vcr: &'a VcrContext,
}

/// Run a single interactive session. Returns the stored messages for inspection.
pub async fn run<W: Write>(
    mut config: RunConfig,
    io: &mut Io,
    vcr: &VcrContext,
    writer: W,
) -> Result<Vec<StoredMessage>> {
    let (mut renderer, mut input) = setup_display(writer, config.term_width, config.show_thinking);
    let mut state = SessionState::default();
    let _raw = RawModeGuard::acquire(vcr.is_live())?;
    renderer.render_hints(crate::display::renderer::HintContext::Initial { has_wait: false });

    let mut append_system_prompt: Option<String> = None;
    if config.fork {
        config.extra_args.extend(ForkConfig::disallowed_tool_args());
        append_system_prompt = Some(fork::fork_system_prompt().to_string());
    }
    if config.reload {
        reload::append_reload_prompt(&mut append_system_prompt);
    }
    let fork_config = ForkConfig::if_enabled(config.fork, &config.extra_args, &config.working_dir);

    let base_session_cfg = SessionConfig {
        extra_args: config.extra_args.clone(),
        append_system_prompt,
        working_dir: config.working_dir.clone(),
        ..Default::default()
    };

    let mut ctx = Ctx {
        input: &mut input,
        renderer: &mut renderer,
        io,
        vcr,
    };

    let Some(mut runner) =
        get_initial_runner(&config, &base_session_cfg, &mut state, &mut ctx).await?
    else {
        return Ok(vec![]);
    };
    let mut watched_tags = Vec::new();
    if config.fork {
        watched_tags.push("fork".to_string());
    }
    if config.reload {
        watched_tags.push("reload".to_string());
    }
    let features = SessionFeatures {
        fork_config: fork_config.as_ref(),
        reload_enabled: config.reload,
        base_config: &base_session_cfg,
        watched_tags,
    };
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

        let resumed = handle_outcome(
            outcome,
            &base_session_cfg,
            &mut runner,
            &mut state,
            &mut ctx,
        )
        .await?;
        if !resumed {
            break;
        }
    }

    runner.close_input();
    let _ = runner.wait().await;
    Ok(renderer.into_messages())
}

/// Handle a session outcome. Returns `true` if the session was resumed.
async fn handle_outcome<W: Write>(
    outcome: SessionOutcome,
    base_session_cfg: &SessionConfig,
    runner: &mut SessionRunner,
    state: &mut SessionState,
    ctx: &mut Ctx<'_, W>,
) -> Result<bool> {
    match outcome {
        SessionOutcome::Completed { .. } => {
            match event_loop::wait_for_followup(
                ctx.input,
                ctx.renderer,
                runner,
                state,
                ctx.io,
                ctx.vcr,
            )
            .await?
            {
                FollowUpAction::Sent => Ok(true),
                FollowUpAction::Interactive => {
                    runner.close_input();
                    let _ = runner.wait().await;
                    let Some(session_id) = state.session_id.take() else {
                        return Ok(false);
                    };
                    let interactive_cfg = SessionConfig {
                        resume: Some(session_id.clone()),
                        ..base_session_cfg.clone()
                    };
                    event_loop::open_interactive_session(&interactive_cfg, ctx.io, ctx.vcr)?;
                    ctx.renderer.render_returned_from_interactive();
                    resume_after_pause(session_id, base_session_cfg, runner, state, ctx).await
                }
                FollowUpAction::Exit => Ok(false),
            }
        }
        SessionOutcome::Interrupted => {
            runner.close_input();
            let _ = runner.wait().await;
            let Some(session_id) = state.session_id.take() else {
                return Ok(false);
            };
            ctx.renderer.render_interrupted();
            resume_after_pause(session_id, base_session_cfg, runner, state, ctx).await
        }
        SessionOutcome::Reload { .. } => {
            runner.kill().await?;
            let Some(session_id) = state.session_id.take() else {
                return Ok(false);
            };
            let (new_runner, new_state) = reload::spawn_reload_session(
                session_id,
                base_session_cfg,
                ctx.renderer,
                ctx.io,
                ctx.vcr,
            )
            .await?;
            *runner = new_runner;
            *state = new_state;
            Ok(true)
        }
        SessionOutcome::ProcessExited => Ok(false),
    }
}

/// Get the initial runner: either from prompt or by waiting for interactive input.
/// Returns None if the user exits without submitting.
async fn get_initial_runner<W: Write>(
    config: &RunConfig,
    base_session_cfg: &SessionConfig,
    state: &mut SessionState,
    ctx: &mut Ctx<'_, W>,
) -> Result<Option<SessionRunner>> {
    if let Some(prompt) = &config.prompt {
        let session_cfg = SessionConfig {
            prompt: Some(prompt.clone()),
            ..base_session_cfg.clone()
        };
        let runner = event_loop::spawn_session(session_cfg, ctx.io, ctx.vcr).await?;
        state.status = SessionStatus::Running;
        return Ok(Some(runner));
    }

    // No prompt — wait for user input or Ctrl+O to open the native TUI.
    match event_loop::wait_for_user_input(ctx.input, ctx.renderer, ctx.io, ctx.vcr).await? {
        Some(event_loop::WaitResult::Text(text)) => {
            let session_cfg = SessionConfig {
                prompt: Some(text),
                ..base_session_cfg.clone()
            };
            let runner = event_loop::spawn_session(session_cfg, ctx.io, ctx.vcr).await?;
            state.status = SessionStatus::Running;
            Ok(Some(runner))
        }
        Some(event_loop::WaitResult::Interactive) => {
            let session_id =
                event_loop::open_interactive_session(base_session_cfg, ctx.io, ctx.vcr)?;
            ctx.renderer.render_returned_from_interactive();
            // The TUI created a session — wait for follow-up text to resume it.
            // (wait_for_interrupt_input handles further Ctrl+O presses internally.)
            let Some(text) = event_loop::wait_for_interrupt_input(
                ctx.input,
                ctx.renderer,
                ctx.io,
                ctx.vcr,
                &session_id,
                base_session_cfg,
            )
            .await?
            else {
                return Ok(None);
            };
            let session_cfg = base_session_cfg.resume_with(text, session_id.clone());
            let runner = event_loop::spawn_session(session_cfg, ctx.io, ctx.vcr).await?;
            state.status = SessionStatus::Running;
            state.session_id = Some(session_id);
            Ok(Some(runner))
        }
        None => Ok(None),
    }
}

/// Wait for user input after a pause (interrupt or interactive), then resume.
/// Returns `true` if resumed, `false` if the user exited.
async fn resume_after_pause<W: Write>(
    session_id: String,
    base_session_cfg: &SessionConfig,
    runner: &mut SessionRunner,
    state: &mut SessionState,
    ctx: &mut Ctx<'_, W>,
) -> Result<bool> {
    let Some(text) = event_loop::wait_for_interrupt_input(
        ctx.input,
        ctx.renderer,
        ctx.io,
        ctx.vcr,
        &session_id,
        base_session_cfg,
    )
    .await?
    else {
        return Ok(false);
    };
    let session_cfg = base_session_cfg.resume_with(text, session_id.clone());
    *runner = event_loop::spawn_session(session_cfg, ctx.io, ctx.vcr).await?;
    *state = SessionState::default();
    state.session_id = Some(session_id);
    Ok(true)
}
