use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::Event;
use crossterm::terminal;

use crate::display::input::{InputAction, InputHandler};
use crate::display::renderer::{Renderer, StoredMessage};
use crate::session::runner::{SessionConfig, SessionRunner};
use crate::session::state::{SessionState, SessionStatus};
use crate::vcr::{Io, IoEvent, VcrContext};

use super::session_loop::{self, FollowUpAction, SessionOutcome};

/// Run a single interactive session. Returns the stored messages for inspection.
pub async fn run<W: Write>(
    prompt: Option<String>,
    extra_args: Vec<String>,
    show_thinking: bool,
    working_dir: Option<PathBuf>,
    io: &mut Io,
    vcr: &VcrContext,
    writer: W,
) -> Result<Vec<StoredMessage>> {
    let mut renderer = Renderer::with_writer(writer);
    renderer.set_show_thinking(show_thinking);
    let mut input = InputHandler::new();
    let mut state = SessionState::default();
    if vcr.is_live() {
        terminal::enable_raw_mode()?;
    }
    renderer.render_help();

    // Get initial runner: either from prompt or by waiting for user input
    let mut runner = if let Some(prompt) = prompt {
        let config = SessionConfig {
            prompt: Some(prompt),
            extra_args: extra_args.clone(),
            working_dir: working_dir.clone(),
            ..Default::default()
        };
        vcr.call("spawn", config, async |c: &SessionConfig| {
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
            &extra_args,
            working_dir.as_ref(),
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
        let outcome =
            session_loop::run_session(&mut runner, &mut state, &mut renderer, &mut input, io, vcr)
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
                let config = SessionConfig {
                    prompt: Some(text),
                    extra_args: extra_args.clone(),
                    resume: Some(session_id),
                    working_dir: working_dir.clone(),
                    ..Default::default()
                };
                runner = vcr
                    .call("spawn", config, async |c: &SessionConfig| {
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
    extra_args: &[String],
    working_dir: Option<&PathBuf>,
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
                        let config = SessionConfig {
                            prompt: Some(text),
                            extra_args: extra_args.to_vec(),
                            working_dir: working_dir.cloned(),
                            ..Default::default()
                        };
                        let runner = vcr
                            .call("spawn", config, async |c: &SessionConfig| {
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
