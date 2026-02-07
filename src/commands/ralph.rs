use anyhow::Result;
use crossterm::event::EventStream;
use crossterm::terminal;
use tokio::sync::mpsc;

use coven::display::input::InputHandler;
use coven::display::renderer::Renderer;
use coven::event::AppEvent;
use coven::session::runner::{SessionConfig, SessionRunner};
use coven::session::state::SessionState;

use super::session_loop::{self, SessionOutcome};

pub struct RalphConfig {
    pub prompt: String,
    pub iterations: u32,
    pub break_tag: String,
    pub no_break: bool,
    pub show_thinking: bool,
    pub extra_args: Vec<String>,
}

/// Run ralph loop mode.
pub async fn ralph(config: RalphConfig) -> Result<()> {
    terminal::enable_raw_mode()?;

    let mut renderer = Renderer::new();
    renderer.set_show_thinking(config.show_thinking);
    renderer.render_help();
    let mut input = InputHandler::new();
    let mut total_cost = 0.0;
    let mut iteration = 0;

    let mut term_events = EventStream::new();

    let system_prompt = if config.no_break {
        "You are running in a loop where each iteration starts a fresh session but the filesystem \
         persists."
            .to_string()
    } else {
        SessionRunner::ralph_system_prompt(&config.break_tag)
    };

    loop {
        iteration += 1;

        // Check iteration limit
        if config.iterations > 0 && iteration > config.iterations {
            renderer.write_raw(&format!(
                "\r\nReached iteration limit ({})\r\n",
                config.iterations
            ));
            break;
        }

        // Iteration header
        renderer.write_raw(&format!("\r\n--- Iteration {iteration} ---\r\n\r\n"));

        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

        let session_config = SessionConfig {
            prompt: Some(config.prompt.clone()),
            extra_args: config.extra_args.clone(),
            append_system_prompt: Some(system_prompt.clone()),
        };

        let mut runner = SessionRunner::spawn(session_config, event_tx).await?;
        let mut state = SessionState::default();

        let outcome = session_loop::run_session(
            &mut runner,
            &mut state,
            &mut renderer,
            &mut input,
            &mut event_rx,
            &mut term_events,
        )
        .await?;

        runner.close_input();
        let _ = runner.wait().await;

        match outcome {
            SessionOutcome::Completed { result_text } => {
                total_cost += state.total_cost_usd;

                // Show running cost
                renderer.write_raw(&format!("  Total cost: ${total_cost:.2}\r\n"));

                // Check for break tag
                if !config.no_break
                    && let Some(reason) =
                        SessionRunner::scan_break_tag(&result_text, &config.break_tag)
                {
                    renderer.write_raw(&format!("\r\nLoop complete: {reason}\r\n"));
                    break;
                }
            }
            SessionOutcome::Interrupted | SessionOutcome::ProcessExited => break,
        }
    }

    terminal::disable_raw_mode()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use coven::session::runner::SessionRunner;

    #[test]
    fn scan_break_tag_found() {
        let text = "I've completed the task. <break>All bugs are fixed.</break> Done.";
        assert_eq!(
            SessionRunner::scan_break_tag(text, "break"),
            Some("All bugs are fixed.".to_string())
        );
    }

    #[test]
    fn scan_break_tag_custom() {
        let text = "Done! <done>Everything works</done>";
        assert_eq!(
            SessionRunner::scan_break_tag(text, "done"),
            Some("Everything works".to_string())
        );
    }

    #[test]
    fn scan_break_tag_not_found() {
        let text = "Still working on the bugs.";
        assert_eq!(SessionRunner::scan_break_tag(text, "break"), None);
    }

    #[test]
    fn scan_break_tag_partial() {
        let text = "Found <break> but no closing tag";
        assert_eq!(SessionRunner::scan_break_tag(text, "break"), None);
    }
}
