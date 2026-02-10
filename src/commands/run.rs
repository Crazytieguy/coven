use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::Event;
use crossterm::terminal;

use crate::display::input::{InputAction, InputHandler};
use crate::display::renderer::{Renderer, StoredMessage};
use crate::fork::{self, ForkConfig};
use crate::session::runner::{SessionConfig, SessionRunner};
use crate::session::state::{SessionState, SessionStatus};
use crate::vcr::{Io, IoEvent, VcrContext};

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
    config: RunConfig,
    io: &mut Io,
    vcr: &VcrContext,
    writer: W,
) -> Result<Vec<StoredMessage>> {
    let mut renderer = Renderer::with_writer(writer);
    renderer.set_show_thinking(config.show_thinking);
    let mut input = InputHandler::new();
    let mut state = SessionState::default();
    if vcr.is_live() {
        terminal::enable_raw_mode()?;
    }
    renderer.render_help();

    let fork_system_prompt = config.fork.then(|| fork::fork_system_prompt().to_string());
    let fork_config = ForkConfig::if_enabled(config.fork, &config.extra_args, &config.working_dir);

    // Get initial runner: either from prompt or by waiting for user input
    let mut runner = if let Some(prompt) = config.prompt {
        let session_cfg = SessionConfig {
            prompt: Some(prompt),
            extra_args: config.extra_args.clone(),
            append_system_prompt: fork_system_prompt.clone(),
            working_dir: config.working_dir.clone(),
            ..Default::default()
        };
        vcr.call("spawn", session_cfg, async |c: &SessionConfig| {
            let tx = io.replace_event_channel();
            SessionRunner::spawn(c.clone(), tx).await
        })
        .await?
    } else {
        renderer.show_prompt();
        input.activate();
        let Some(runner) = wait_for_initial_prompt(
            &mut input,
            &mut renderer,
            &mut state,
            fork_system_prompt.as_deref(),
            &config,
            io,
            vcr,
        )
        .await?
        else {
            if vcr.is_live() {
                terminal::disable_raw_mode()?;
            }
            return Ok(vec![]);
        };
        runner
    };

    // Main session loop — run sessions with follow-up support
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
                let session_cfg = SessionConfig {
                    prompt: Some(text),
                    extra_args: config.extra_args.clone(),
                    append_system_prompt: fork_system_prompt.clone(),
                    resume: Some(session_id),
                    working_dir: config.working_dir.clone(),
                };
                runner = vcr
                    .call("spawn", session_cfg, async |c: &SessionConfig| {
                        let tx = io.replace_event_channel();
                        SessionRunner::spawn(c.clone(), tx).await
                    })
                    .await?;
                state = SessionState::default();
            }
            SessionOutcome::ProcessExited => break,
        }
    }

    if vcr.is_live() {
        terminal::disable_raw_mode()?;
    }
    runner.close_input();
    let _ = runner.wait().await;
    Ok(renderer.into_messages())
}

/// Wait for the user to type an initial prompt. Returns the spawned runner, or None to exit.
async fn wait_for_initial_prompt<W: Write>(
    input: &mut InputHandler,
    renderer: &mut Renderer<W>,
    state: &mut SessionState,
    fork_system_prompt: Option<&str>,
    run_config: &RunConfig,
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<Option<SessionRunner>> {
    loop {
        let io_event: IoEvent = vcr
            .call("next_event", (), async |(): &()| io.next_event().await)
            .await?;
        match io_event {
            IoEvent::Terminal(Event::Key(key_event)) => {
                let action = input.handle_key(&key_event);
                match action {
                    InputAction::Submit(text, _) => {
                        let session_cfg = SessionConfig {
                            prompt: Some(text),
                            extra_args: run_config.extra_args.clone(),
                            append_system_prompt: fork_system_prompt.map(String::from),
                            working_dir: run_config.working_dir.clone(),
                            ..Default::default()
                        };
                        let runner = vcr
                            .call("spawn", session_cfg, async |c: &SessionConfig| {
                                let tx = io.replace_event_channel();
                                SessionRunner::spawn(c.clone(), tx).await
                            })
                            .await?;
                        state.status = SessionStatus::Running;
                        return Ok(Some(runner));
                    }
                    InputAction::Interrupt | InputAction::EndSession => return Ok(None),
                    InputAction::Cancel => {
                        renderer.show_prompt();
                        input.activate();
                    }
                    _ => {}
                }
            }
            IoEvent::Terminal(_) | IoEvent::Claude(_) => {}
        }
    }
}
