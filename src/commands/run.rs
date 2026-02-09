use anyhow::Result;
use crossterm::event::{Event, EventStream};
use crossterm::terminal;
use futures::StreamExt;
use tokio::sync::mpsc;

use coven::display::input::{InputAction, InputHandler};
use coven::display::renderer::Renderer;
use coven::event::AppEvent;
use coven::session::runner::{SessionConfig, SessionRunner};
use coven::session::state::{SessionState, SessionStatus};

use super::session_loop::{self, FollowUpAction, SessionOutcome};

/// Run a single interactive session.
pub async fn run(
    prompt: Option<String>,
    extra_args: Vec<String>,
    show_thinking: bool,
) -> Result<()> {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();
    let mut renderer = Renderer::new();
    renderer.set_show_thinking(show_thinking);
    let mut input = InputHandler::new();
    let mut state = SessionState::default();
    let mut term_events = EventStream::new();
    terminal::enable_raw_mode()?;
    renderer.render_help();

    // Get initial runner: either from prompt or by waiting for user input
    let mut runner = if let Some(prompt) = prompt {
        let config = SessionConfig {
            prompt: Some(prompt),
            extra_args: extra_args.clone(),
            ..Default::default()
        };
        SessionRunner::spawn(config, event_tx).await?
    } else {
        renderer.show_prompt();
        input.activate();
        if let Some(runner) = wait_for_initial_prompt(
            &mut input,
            &mut renderer,
            &mut state,
            &event_tx,
            &extra_args,
            &mut term_events,
        )
        .await?
        {
            runner
        } else {
            terminal::disable_raw_mode()?;
            return Ok(());
        }
    };

    // Main session loop — run sessions with follow-up support
    loop {
        let outcome = session_loop::run_session(
            &mut runner,
            &mut state,
            &mut renderer,
            &mut input,
            &mut event_rx,
            &mut term_events,
        )
        .await?;

        match outcome {
            SessionOutcome::Completed { .. } => {
                match session_loop::wait_for_followup(
                    &mut input,
                    &mut renderer,
                    &mut runner,
                    &mut state,
                    &mut term_events,
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
                // If we have a session_id, offer to resume; otherwise just exit
                if let Some(session_id) = state.session_id.take() {
                    renderer.render_interrupted();

                    match session_loop::wait_for_user_input(
                        &mut input,
                        &mut renderer,
                        &mut term_events,
                    )
                    .await?
                    {
                        Some(text) => {
                            let (new_tx, new_rx) = mpsc::unbounded_channel();
                            event_rx = new_rx;
                            let config = SessionConfig {
                                prompt: Some(text),
                                extra_args: extra_args.clone(),
                                resume: Some(session_id),
                                ..Default::default()
                            };
                            runner = SessionRunner::spawn(config, new_tx).await?;
                            state = SessionState::default();
                        }
                        None => break,
                    }
                } else {
                    break;
                }
            }
            SessionOutcome::ProcessExited => break,
        }
    }

    terminal::disable_raw_mode()?;
    runner.close_input();
    let _ = runner.wait().await;
    Ok(())
}

/// Wait for the user to type an initial prompt. Returns the spawned runner, or None to exit.
async fn wait_for_initial_prompt(
    input: &mut InputHandler,
    renderer: &mut Renderer,
    state: &mut SessionState,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
    extra_args: &[String],
    term_events: &mut EventStream,
) -> Result<Option<SessionRunner>> {
    loop {
        match term_events.next().await {
            Some(Ok(Event::Key(key_event))) => {
                let action = input.handle_key(&key_event);
                match action {
                    InputAction::Submit(text, _) => {
                        let config = SessionConfig {
                            prompt: Some(text),
                            extra_args: extra_args.to_vec(),
                            ..Default::default()
                        };
                        let runner = SessionRunner::spawn(config, event_tx.clone()).await?;
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
            Some(Ok(_)) => {}
            Some(Err(_)) | None => return Ok(None),
        }
    }
}
