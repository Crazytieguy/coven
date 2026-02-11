use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;

use crate::display::input::InputHandler;
use crate::display::renderer::{Renderer, StoredMessage};
use crate::fork::{self, ForkConfig};
use crate::session::runner::{SessionConfig, SessionRunner};
use crate::session::state::{SessionState, SessionStatus};
use crate::vcr::{Io, VcrContext};

use super::RawModeGuard;
use super::session_loop::{self, FollowUpAction, SessionOutcome};

pub struct RunConfig {
    pub prompt: Option<String>,
    pub extra_args: Vec<String>,
    pub show_thinking: bool,
    pub fork: bool,
    pub working_dir: Option<PathBuf>,
}

/// Run a single interactive session. Returns the stored messages for inspection.
pub async fn run<W: Write>(
    mut config: RunConfig,
    io: &mut Io,
    vcr: &VcrContext,
    writer: W,
) -> Result<Vec<StoredMessage>> {
    let mut renderer = Renderer::with_writer(writer);
    renderer.set_show_thinking(config.show_thinking);
    let mut input = InputHandler::new(2);
    let mut state = SessionState::default();
    let _raw = RawModeGuard::acquire(vcr.is_live())?;
    renderer.render_help();

    let fork_system_prompt = config.fork.then(|| fork::fork_system_prompt().to_string());
    if config.fork {
        config.extra_args.extend(ForkConfig::disallowed_tool_args());
    }
    let fork_config = ForkConfig::if_enabled(config.fork, &config.extra_args, &config.working_dir);

    let base_session_cfg = SessionConfig {
        extra_args: config.extra_args.clone(),
        append_system_prompt: fork_system_prompt,
        working_dir: config.working_dir.clone(),
        ..Default::default()
    };

    let Some(mut runner) = get_initial_runner(
        config.prompt.as_deref(),
        &base_session_cfg,
        &mut renderer,
        &mut input,
        &mut state,
        io,
        vcr,
    )
    .await?
    else {
        return Ok(vec![]);
    };
    loop {
        let outcome = session_loop::run_session(
            &mut runner,
            &mut state,
            &mut renderer,
            &mut input,
            io,
            vcr,
            fork_config.as_ref(),
        )
        .await?;

        match outcome {
            SessionOutcome::Completed { .. } => {
                match session_loop::wait_for_followup(
                    &mut input,
                    &mut renderer,
                    &mut runner,
                    &mut state,
                    io,
                    vcr,
                )
                .await?
                {
                    FollowUpAction::Sent => {}
                    FollowUpAction::Exit => break,
                }
            }
            SessionOutcome::Interrupted => {
                runner.close_input();
                let _ = runner.wait().await;
                io.clear_event_channel();
                let Some(session_id) = state.session_id.take() else {
                    break;
                };
                renderer.render_interrupted();
                let Some(text) =
                    session_loop::wait_for_user_input(&mut input, &mut renderer, io, vcr).await?
                else {
                    break;
                };
                let session_cfg = base_session_cfg.resume_with(text, session_id);
                runner = session_loop::spawn_session(session_cfg, io, vcr).await?;
                let prev_session_id = state.session_id.clone();
                state = SessionState::default();
                state.session_id = prev_session_id;
            }
            SessionOutcome::ProcessExited => break,
        }
    }

    runner.close_input();
    let _ = runner.wait().await;
    Ok(renderer.into_messages())
}

/// Get the initial runner: either from prompt or by waiting for interactive input.
/// Returns None if the user exits without submitting.
async fn get_initial_runner<W: Write>(
    prompt: Option<&str>,
    base_session_cfg: &SessionConfig,
    renderer: &mut Renderer<W>,
    input: &mut InputHandler,
    state: &mut SessionState,
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<Option<SessionRunner>> {
    let text = if let Some(prompt) = prompt {
        prompt.to_string()
    } else {
        let Some(text) = session_loop::wait_for_user_input(input, renderer, io, vcr).await? else {
            return Ok(None);
        };
        text
    };

    let session_cfg = SessionConfig {
        prompt: Some(text),
        ..base_session_cfg.clone()
    };
    let runner = session_loop::spawn_session(session_cfg, io, vcr).await?;
    state.status = SessionStatus::Running;
    Ok(Some(runner))
}
