use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;

use crate::display::input::InputHandler;
use crate::display::renderer::{Renderer, StoredMessage};
use crate::fork::{self, ForkConfig};
use crate::session::runner::{SessionConfig, SessionRunner};
use crate::session::state::SessionState;
use crate::vcr::{Io, VcrContext};

use super::RawModeGuard;
use super::session_loop::{self, SessionOutcome};

pub struct RalphConfig {
    pub prompt: String,
    pub iterations: u32,
    pub break_tag: String,
    pub no_break: bool,
    pub show_thinking: bool,
    pub fork: bool,
    pub extra_args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    /// Override terminal width for display truncation (used in tests).
    pub term_width: Option<usize>,
}

impl RalphConfig {
    fn system_prompt(&self) -> String {
        let base = if self.no_break {
            "You are running in a loop where each iteration starts a fresh session but the \
             filesystem persists."
                .to_string()
        } else {
            SessionRunner::ralph_system_prompt(&self.break_tag)
        };
        if self.fork {
            format!("{base}\n\n{}", fork::fork_system_prompt())
        } else {
            base
        }
    }
}

/// Run ralph loop mode.
pub async fn ralph<W: Write>(
    mut config: RalphConfig,
    io: &mut Io,
    vcr: &VcrContext,
    writer: W,
) -> Result<Vec<StoredMessage>> {
    let _raw = RawModeGuard::acquire(vcr.is_live())?;

    let mut renderer = Renderer::with_writer(writer);
    if let Some(w) = config.term_width {
        renderer.set_width(w);
    }
    renderer.set_show_thinking(config.show_thinking);
    renderer.render_help();
    let mut input = InputHandler::new(2);
    let mut total_cost = 0.0;
    let mut iteration = 0;
    let system_prompt = config.system_prompt();
    if config.fork {
        config.extra_args.extend(ForkConfig::disallowed_tool_args());
    }
    let fork_config = ForkConfig::if_enabled(config.fork, &config.extra_args, &config.working_dir);

    'outer: loop {
        iteration += 1;
        if config.iterations > 0 && iteration > config.iterations {
            renderer.write_raw(&format!(
                "\r\nReached iteration limit ({})\r\n",
                config.iterations
            ));
            break;
        }

        // Iteration header
        renderer.write_raw(&format!("\r\n--- Iteration {iteration} ---\r\n\r\n"));

        let session_config = SessionConfig {
            prompt: Some(config.prompt.clone()),
            extra_args: config.extra_args.clone(),
            append_system_prompt: Some(system_prompt.clone()),
            working_dir: config.working_dir.clone(),
            ..Default::default()
        };

        let mut runner = session_loop::spawn_session(session_config.clone(), io, vcr).await?;
        let mut state = SessionState::default();
        let mut iteration_cost = 0.0;

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
            runner.close_input();
            let _ = runner.wait().await;

            match outcome {
                SessionOutcome::Completed { result_text } => {
                    iteration_cost += state.total_cost_usd;
                    total_cost += iteration_cost;
                    renderer.write_raw(&format!("  Total cost: ${total_cost:.2}\r\n"));

                    if !config.no_break
                        && let Some(reason) =
                            SessionRunner::scan_break_tag(&result_text, &config.break_tag)
                    {
                        let s = if iteration == 1 { "" } else { "s" };
                        renderer.write_raw(&format!(
                            "\r\nLoop complete ({iteration} iteration{s}): {reason}\r\n"
                        ));
                        break 'outer;
                    }

                    break; // next iteration
                }
                SessionOutcome::Interrupted => {
                    io.clear_event_channel();
                    let Some(session_id) = state.session_id.take() else {
                        break 'outer;
                    };
                    iteration_cost += state.total_cost_usd;
                    renderer.render_interrupted();

                    let Some(text) = session_loop::wait_for_interrupt_input(
                        &mut input,
                        &mut renderer,
                        io,
                        vcr,
                        &session_id,
                        config.working_dir.as_deref(),
                        &config.extra_args,
                    )
                    .await?
                    else {
                        break 'outer;
                    };
                    let resume_config = session_config.resume_with(text, session_id.clone());
                    runner = session_loop::spawn_session(resume_config, io, vcr).await?;
                    state = SessionState::default();
                    state.session_id = Some(session_id);
                }
                SessionOutcome::ProcessExited => break 'outer,
            }
        }
    }

    Ok(renderer.into_messages())
}

#[cfg(test)]
mod tests {
    use crate::session::runner::SessionRunner;

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
