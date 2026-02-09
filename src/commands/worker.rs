use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use crossterm::event::{Event, EventStream};
use crossterm::terminal;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::time::sleep;

use coven::agents::{self, AgentDef};
use coven::dispatch::{self, DispatchDecision};
use coven::display::input::{InputAction, InputHandler};
use coven::display::renderer::Renderer;
use coven::event::AppEvent;
use coven::session::runner::{SessionConfig, SessionRunner};
use coven::session::state::SessionState;
use coven::worker_state;
use coven::worktree::{self, SpawnOptions};

use super::session_loop::{self, SessionOutcome};

pub struct WorkerConfig {
    pub show_thinking: bool,
    pub branch: Option<String>,
    pub worktree_base: PathBuf,
    pub extra_args: Vec<String>,
}

/// Run a worker: spawn a worktree, loop dispatch → agent → land.
pub async fn worker(config: WorkerConfig) -> Result<()> {
    let project_root = std::env::current_dir()?;

    let spawn_result = worktree::spawn(&SpawnOptions {
        repo_path: &project_root,
        branch: config.branch.as_deref(),
        base_path: &config.worktree_base,
    })?;

    terminal::enable_raw_mode()?;
    let mut renderer = Renderer::new();
    renderer.set_show_thinking(config.show_thinking);
    renderer.render_help();
    let mut input = InputHandler::new();
    let mut term_events = EventStream::new();
    let mut total_cost = 0.0;

    worker_state::register(&spawn_result.worktree_path)?;

    renderer.write_raw(&format!(
        "\r\nWorker started: {} ({})\r\n",
        spawn_result.branch,
        spawn_result.worktree_path.display()
    ));

    let result = worker_loop(
        &config,
        &spawn_result.worktree_path,
        &mut renderer,
        &mut input,
        &mut term_events,
        &mut total_cost,
    )
    .await;

    terminal::disable_raw_mode()?;

    worker_state::deregister(&spawn_result.worktree_path);

    renderer.write_raw("\r\nRemoving worktree...\r\n");
    if let Err(e) = worktree::remove(&spawn_result.worktree_path) {
        renderer.write_raw(&format!("Warning: failed to remove worktree: {e}\r\n"));
    }

    result
}

/// Outcome of the dispatch phase.
struct DispatchResult {
    decision: DispatchDecision,
    agent_defs: Vec<AgentDef>,
    cost: f64,
}

/// Run the dispatch phase: load agents, run dispatch session, parse decision.
async fn run_dispatch(
    worktree_path: &Path,
    extra_args: &[String],
    worker_status: &str,
    renderer: &mut Renderer,
    input: &mut InputHandler,
    term_events: &mut EventStream,
) -> Result<Option<DispatchResult>> {
    renderer.write_raw("\r\n=== Dispatch ===\r\n\r\n");

    let agents_dir = worktree_path.join(agents::AGENTS_DIR);
    let agent_defs = agents::load_agents(&agents_dir)?;
    if agent_defs.is_empty() {
        bail!("no agent definitions found in {}", agents_dir.display());
    }

    let dispatch_agent = agent_defs
        .iter()
        .find(|a| a.name == "dispatch")
        .context("no dispatch.md agent definition found")?;

    let catalog = dispatch::format_agent_catalog(&agent_defs);
    let dispatch_args = HashMap::from([
        ("agent_catalog".to_string(), catalog),
        ("worker_status".to_string(), worker_status.to_string()),
    ]);

    let dispatch_prompt = dispatch_agent.render(&dispatch_args)?;

    match run_phase_session(
        &dispatch_prompt,
        worktree_path,
        extra_args,
        renderer,
        input,
        term_events,
    )
    .await?
    {
        PhaseOutcome::Completed { result_text, cost } => {
            let decision = dispatch::parse_decision(&result_text)
                .context("failed to parse dispatch decision")?;
            Ok(Some(DispatchResult {
                decision,
                agent_defs,
                cost,
            }))
        }
        PhaseOutcome::Exited => Ok(None),
    }
}

async fn worker_loop(
    config: &WorkerConfig,
    worktree_path: &Path,
    renderer: &mut Renderer,
    input: &mut InputHandler,
    term_events: &mut EventStream,
    total_cost: &mut f64,
) -> Result<()> {
    loop {
        // Sync worktree to latest main so dispatch sees current issue state
        worktree::sync_to_main(worktree_path).context("failed to sync worktree to main")?;

        // === Phase 1: Dispatch (under lock) ===
        let lock = worker_state::acquire_dispatch_lock(worktree_path)?;
        let all_workers = worker_state::read_all(worktree_path)?;
        let worker_status = worker_state::format_status(&all_workers);

        let Some(dispatch) = run_dispatch(
            worktree_path,
            &config.extra_args,
            &worker_status,
            renderer,
            input,
            term_events,
        )
        .await?
        else {
            return Ok(());
        };

        drop(lock);

        *total_cost += dispatch.cost;
        renderer.write_raw(&format!("  Total cost: ${total_cost:.2}\r\n"));

        match dispatch.decision {
            DispatchDecision::Sleep => {
                worker_state::update(worktree_path, None, &HashMap::new())?;
                renderer.write_raw("\r\nDispatch: sleep — waiting for new commits...\r\n");
                match wait_for_new_commits(worktree_path, renderer, input, term_events).await? {
                    WaitOutcome::NewCommits => {}
                    WaitOutcome::Exited => return Ok(()),
                }
            }
            DispatchDecision::RunAgent { agent, args } => {
                worker_state::update(worktree_path, Some(&agent), &args)?;

                let args_display = args
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                renderer.write_raw(&format!("\r\nDispatch: {agent} {args_display}\r\n"));

                // === Phase 2: Run Agent ===
                let agent_def = dispatch
                    .agent_defs
                    .iter()
                    .find(|a| a.name == agent)
                    .with_context(|| format!("dispatch chose unknown agent: {agent}"))?;

                let agent_prompt = agent_def.render(&args)?;
                renderer.write_raw(&format!("\r\n=== Agent: {agent} ===\r\n\r\n"));

                match run_phase_session(
                    &agent_prompt,
                    worktree_path,
                    &config.extra_args,
                    renderer,
                    input,
                    term_events,
                )
                .await?
                {
                    PhaseOutcome::Completed { cost, .. } => {
                        *total_cost += cost;
                        renderer.write_raw(&format!("  Total cost: ${total_cost:.2}\r\n"));
                    }
                    PhaseOutcome::Exited => return Ok(()),
                }

                // === Phase 3: Land ===
                land_worktree(worktree_path, renderer)?;
            }
        }
    }
}

fn land_worktree(worktree_path: &Path, renderer: &mut Renderer) -> Result<()> {
    renderer.write_raw("\r\n=== Landing ===\r\n");

    match worktree::land(worktree_path) {
        Ok(result) => {
            renderer.write_raw(&format!(
                "Landed {} onto {}\r\n",
                result.branch, result.main_branch
            ));
        }
        Err(worktree::WorktreeError::RebaseConflict(files)) => {
            renderer.write_raw(&format!("Rebase conflict in: {}\r\n", files.join(", ")));
            renderer.write_raw("Aborting rebase, resetting to main.\r\n");
            worktree::abort_rebase(worktree_path)?;
            worktree::reset_to_main(worktree_path)?;
            let _ = worktree::clean(worktree_path);
        }
        Err(e) => {
            renderer.write_raw(&format!("Land failed: {e}\r\n"));
            renderer.write_raw("Resetting to main.\r\n");
            worktree::reset_to_main(worktree_path)?;
            let _ = worktree::clean(worktree_path);
        }
    }

    Ok(())
}

enum PhaseOutcome {
    Completed { result_text: String, cost: f64 },
    Exited,
}

/// Run an interactive claude session for a worker phase (dispatch or agent).
async fn run_phase_session(
    prompt: &str,
    working_dir: &Path,
    extra_args: &[String],
    renderer: &mut Renderer,
    input: &mut InputHandler,
    term_events: &mut EventStream,
) -> Result<PhaseOutcome> {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

    let session_config = SessionConfig {
        prompt: Some(prompt.to_string()),
        extra_args: extra_args.to_vec(),
        working_dir: Some(working_dir.to_path_buf()),
        ..Default::default()
    };

    let mut runner = SessionRunner::spawn(session_config, event_tx).await?;
    let mut state = SessionState::default();

    loop {
        let outcome = session_loop::run_session(
            &mut runner,
            &mut state,
            renderer,
            input,
            &mut event_rx,
            term_events,
        )
        .await?;

        runner.close_input();
        let _ = runner.wait().await;

        match outcome {
            SessionOutcome::Completed { result_text } => {
                return Ok(PhaseOutcome::Completed {
                    result_text,
                    cost: state.total_cost_usd,
                });
            }
            SessionOutcome::Interrupted => {
                let Some(session_id) = state.session_id.take() else {
                    return Ok(PhaseOutcome::Exited);
                };
                renderer.render_interrupted();

                match session_loop::wait_for_user_input(input, renderer, term_events).await? {
                    Some(text) => {
                        let (new_tx, new_rx) = mpsc::unbounded_channel();
                        event_rx = new_rx;
                        let resume_config = SessionConfig {
                            prompt: Some(text),
                            extra_args: extra_args.to_vec(),
                            resume: Some(session_id),
                            working_dir: Some(working_dir.to_path_buf()),
                            ..Default::default()
                        };
                        runner = SessionRunner::spawn(resume_config, new_tx).await?;
                        state = SessionState::default();
                    }
                    None => return Ok(PhaseOutcome::Exited),
                }
            }
            SessionOutcome::ProcessExited => {
                return Ok(PhaseOutcome::Exited);
            }
        }
    }
}

enum WaitOutcome {
    NewCommits,
    Exited,
}

/// Wait for new commits on main by polling, while allowing the user to exit.
async fn wait_for_new_commits(
    worktree_path: &Path,
    renderer: &mut Renderer,
    input: &mut InputHandler,
    term_events: &mut EventStream,
) -> Result<WaitOutcome> {
    let initial_head = main_head_sha(worktree_path)?;

    loop {
        tokio::select! {
            () = sleep(Duration::from_secs(10)) => {
                let current = main_head_sha(worktree_path)?;
                if current != initial_head {
                    renderer.write_raw("New commits detected on main.\r\n");
                    return Ok(WaitOutcome::NewCommits);
                }
            }
            event = term_events.next() => {
                if let Some(Ok(Event::Key(key_event))) = event {
                    let action = input.handle_key(&key_event);
                    if matches!(action, InputAction::Interrupt | InputAction::EndSession) {
                        return Ok(WaitOutcome::Exited);
                    }
                }
            }
        }
    }
}

/// Get the SHA of the main branch's HEAD.
fn main_head_sha(worktree_path: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree_path)
        .args(["worktree", "list", "--porcelain"])
        .output()
        .context("failed to run git worktree list")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let main_branch = stdout
        .lines()
        .find_map(|line| line.strip_prefix("branch refs/heads/"))
        .context("could not find main branch in worktree list")?;

    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree_path)
        .args(["rev-parse", main_branch])
        .output()
        .context("failed to run git rev-parse")?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
