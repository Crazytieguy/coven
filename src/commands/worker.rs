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
pub async fn worker(mut config: WorkerConfig) -> Result<()> {
    // Default to acceptEdits (same as other commands) unless the user
    // specified a permission mode. The user is expected to set up persistent
    // permissions for their project so agents can run unattended.
    if !config.extra_args.iter().any(|a| a == "--permission-mode") {
        config
            .extra_args
            .extend(["--permission-mode".to_string(), "acceptEdits".to_string()]);
    }

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

    worker_state::register(&spawn_result.worktree_path, &spawn_result.branch)?;

    renderer.set_title(&format!("coven: {}", spawn_result.branch));
    renderer.write_raw(&format!(
        "\r\nWorker started: {} ({})\r\n",
        spawn_result.branch,
        spawn_result.worktree_path.display()
    ));

    let result = worker_loop(
        &config,
        &spawn_result.worktree_path,
        &spawn_result.branch,
        &mut renderer,
        &mut input,
        &mut term_events,
        &mut total_cost,
    )
    .await;

    terminal::disable_raw_mode()?;
    renderer.set_title("");

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
    branch: &str,
    extra_args: &[String],
    worker_status: &str,
    renderer: &mut Renderer,
    input: &mut InputHandler,
    term_events: &mut EventStream,
) -> Result<Option<DispatchResult>> {
    renderer.set_title(&format!("coven: {branch} \u{2014} dispatch"));
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

    // Run the dispatch session
    let PhaseOutcome::Completed {
        result_text,
        cost,
        session_id,
    } = run_phase_session(
        &dispatch_prompt,
        worktree_path,
        extra_args,
        None,
        renderer,
        input,
        term_events,
    )
    .await?
    else {
        return Ok(None);
    };

    // Try to parse the decision
    match dispatch::parse_decision(&result_text) {
        Ok(decision) => Ok(Some(DispatchResult {
            decision,
            agent_defs,
            cost,
        })),
        Err(parse_err) => {
            // If we have a session to resume, retry with a correction prompt
            let Some(session_id) = session_id else {
                return Err(parse_err).context("failed to parse dispatch decision");
            };

            renderer.write_raw(&format!(
                "\r\nDispatch output could not be parsed: {parse_err}\r\nRetrying...\r\n\r\n"
            ));

            let retry_prompt = format!(
                "Your previous output could not be parsed: {parse_err}\n\n\
                 Please output your decision inside a <dispatch> tag containing YAML. \
                 For example:\n\n\
                 <dispatch>\nagent: plan\nissue: issues/example.md\n</dispatch>\n\n\
                 Or to sleep:\n\n\
                 <dispatch>\nsleep: true\n</dispatch>"
            );

            let PhaseOutcome::Completed {
                result_text: retry_text,
                cost: retry_cost,
                ..
            } = run_phase_session(
                &retry_prompt,
                worktree_path,
                extra_args,
                Some(&session_id),
                renderer,
                input,
                term_events,
            )
            .await?
            else {
                return Ok(None);
            };

            let decision = dispatch::parse_decision(&retry_text)
                .context("failed to parse dispatch decision after retry")?;
            Ok(Some(DispatchResult {
                decision,
                agent_defs,
                cost: cost + retry_cost,
            }))
        }
    }
}

async fn worker_loop(
    config: &WorkerConfig,
    worktree_path: &Path,
    branch: &str,
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
            branch,
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

        // Update worker state before releasing lock so the next dispatch sees it
        match &dispatch.decision {
            DispatchDecision::Sleep => {
                worker_state::update(worktree_path, branch, None, &HashMap::new())?;
            }
            DispatchDecision::RunAgent { agent, args } => {
                worker_state::update(worktree_path, branch, Some(agent), args)?;
            }
        }
        drop(lock);

        *total_cost += dispatch.cost;
        renderer.write_raw(&format!("  Total cost: ${total_cost:.2}\r\n"));

        match dispatch.decision {
            DispatchDecision::Sleep => {
                renderer.set_title(&format!("coven: {branch} \u{2014} sleeping"));
                renderer.write_raw("\r\nDispatch: sleep — waiting for new commits...\r\n");
                match wait_for_new_commits(worktree_path, renderer, input, term_events).await? {
                    WaitOutcome::NewCommits => {}
                    WaitOutcome::Exited => return Ok(()),
                }
            }
            DispatchDecision::RunAgent { agent, args } => {
                let args_display = args
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                renderer.write_raw(&format!("\r\nDispatch: {agent} {args_display}\r\n"));

                let agent_def = dispatch
                    .agent_defs
                    .iter()
                    .find(|a| a.name == agent)
                    .with_context(|| format!("dispatch chose unknown agent: {agent}"))?;

                let agent_prompt = agent_def.render(&args)?;
                renderer.write_raw(&format!("\r\n=== Agent: {agent} ===\r\n\r\n"));
                let title_suffix = if args_display.is_empty() {
                    agent
                } else {
                    format!("{agent} {args_display}")
                };
                renderer.set_title(&format!("coven: {branch} \u{2014} {title_suffix}"));

                let should_exit = run_agent(
                    &agent_prompt,
                    worktree_path,
                    &config.extra_args,
                    renderer,
                    input,
                    term_events,
                    total_cost,
                )
                .await?;
                if should_exit {
                    return Ok(());
                }

                // Clear state so other dispatchers don't see stale agent info
                worker_state::update(worktree_path, branch, None, &HashMap::new())?;
            }
        }
    }
}

/// Run the agent phase: execute the agent session, ensure commits, and land.
/// Returns true if the worker should exit (user interrupted).
async fn run_agent(
    prompt: &str,
    worktree_path: &Path,
    extra_args: &[String],
    renderer: &mut Renderer,
    input: &mut InputHandler,
    term_events: &mut EventStream,
    total_cost: &mut f64,
) -> Result<bool> {
    let agent_session_id = match run_phase_session(
        prompt,
        worktree_path,
        extra_args,
        None,
        renderer,
        input,
        term_events,
    )
    .await?
    {
        PhaseOutcome::Completed {
            cost, session_id, ..
        } => {
            *total_cost += cost;
            renderer.write_raw(&format!("  Total cost: ${total_cost:.2}\r\n"));
            session_id
        }
        PhaseOutcome::Exited => return Ok(true),
    };

    // === Phase 3: Land ===
    // Clean untracked files before landing. Agents should commit
    // their work; leftover files (test artifacts, temp files) must
    // not block landing and cause committed work to be discarded.
    let _ = worktree::clean(worktree_path);

    let commit_result = ensure_commits(
        worktree_path,
        agent_session_id,
        extra_args,
        renderer,
        input,
        term_events,
        total_cost,
    )
    .await?;

    match commit_result {
        CommitCheck::HasCommits { session_id } => {
            let should_exit = land_or_resolve(
                worktree_path,
                session_id.as_deref(),
                extra_args,
                renderer,
                input,
                term_events,
                total_cost,
            )
            .await?;
            if should_exit {
                return Ok(true);
            }
        }
        CommitCheck::NoCommits => {
            renderer.write_raw("Agent produced no commits — skipping land.\r\n");
        }
        CommitCheck::Exited => return Ok(true),
    }

    Ok(false)
}

enum CommitCheck {
    /// Agent has commits ready to land, with the session ID to use for conflict resolution.
    HasCommits { session_id: Option<String> },
    /// Agent produced no commits even after being asked.
    NoCommits,
    /// User exited during the commit prompt.
    Exited,
}

/// Check if the agent produced commits. If not, resume once to ask it to commit.
async fn ensure_commits(
    worktree_path: &Path,
    agent_session_id: Option<String>,
    extra_args: &[String],
    renderer: &mut Renderer,
    input: &mut InputHandler,
    term_events: &mut EventStream,
    total_cost: &mut f64,
) -> Result<CommitCheck> {
    if worktree::has_unique_commits(worktree_path)? {
        return Ok(CommitCheck::HasCommits {
            session_id: agent_session_id,
        });
    }

    let Some(sid) = agent_session_id.as_deref() else {
        renderer.write_raw("Agent produced no commits and no session to resume.\r\n");
        return Ok(CommitCheck::NoCommits);
    };

    renderer.write_raw("Agent produced no commits — resuming session to ask for a commit.\r\n\r\n");

    match run_phase_session(
        "You finished without committing anything. \
         If you have changes worth keeping, please commit them now. \
         If there's nothing to commit, just confirm that.",
        worktree_path,
        extra_args,
        Some(sid),
        renderer,
        input,
        term_events,
    )
    .await?
    {
        PhaseOutcome::Completed {
            cost, session_id, ..
        } => {
            *total_cost += cost;
            renderer.write_raw(&format!("  Total cost: ${total_cost:.2}\r\n"));
            let _ = worktree::clean(worktree_path);

            if worktree::has_unique_commits(worktree_path)? {
                Ok(CommitCheck::HasCommits { session_id })
            } else {
                Ok(CommitCheck::NoCommits)
            }
        }
        PhaseOutcome::Exited => Ok(CommitCheck::Exited),
    }
}

/// Attempt to land and, on rebase conflict, resume the agent session to resolve.
///
/// After successful conflict resolution, retries the full land (rebase + ff-merge)
/// rather than just ff-merge. This handles the case where another worker landed
/// while conflict resolution was in progress, which would cause a bare ff-merge
/// to fail and silently lose the resolved work.
///
/// Returns true if the worker should exit (user interrupted during resolution).
async fn land_or_resolve(
    worktree_path: &Path,
    session_id: Option<&str>,
    extra_args: &[String],
    renderer: &mut Renderer,
    input: &mut InputHandler,
    term_events: &mut EventStream,
    total_cost: &mut f64,
) -> Result<bool> {
    renderer.write_raw("\r\n=== Landing ===\r\n");

    // Track the session to resume for conflict resolution. Starts as the
    // agent's session, then updated to the resolution session's ID so
    // subsequent rounds of conflicts can be resolved in-context.
    let mut resume_session_id = session_id.map(String::from);

    loop {
        let conflict_files = match worktree::land(worktree_path) {
            Ok(result) => {
                renderer.write_raw(&format!(
                    "Landed {} onto {}\r\n",
                    result.branch, result.main_branch
                ));
                return Ok(false);
            }
            Err(worktree::WorktreeError::RebaseConflict(files)) => files,
            Err(e) => {
                renderer.write_raw(&format!("Land failed: {e}\r\n"));
                renderer.write_raw("Resetting to main.\r\n");
                worktree::reset_to_main(worktree_path)?;
                let _ = worktree::clean(worktree_path);
                return Ok(false);
            }
        };

        // Rebase is in progress with conflict markers. Resume the session
        // to resolve, or abort if we don't have a session to resume.
        let files_display = conflict_files.join(", ");

        let Some(sid) = resume_session_id.as_deref() else {
            renderer.write_raw(&format!("Rebase conflict in: {files_display}\r\n"));
            renderer.write_raw("No session to resume — aborting rebase.\r\n");
            worktree::abort_rebase(worktree_path)?;
            worktree::reset_to_main(worktree_path)?;
            let _ = worktree::clean(worktree_path);
            return Ok(false);
        };

        renderer.write_raw(&format!(
            "Rebase conflict in: {files_display} — resuming session to resolve.\r\n"
        ));
        renderer.write_raw("\r\n=== Conflict Resolution ===\r\n\r\n");

        let prompt = format!(
            "The rebase onto main hit conflicts in: {files_display}\n\n\
             Resolve the conflicts in those files, stage them with `git add`, \
             and run `git rebase --continue`. If more conflicts appear after \
             continuing, resolve those too until the rebase completes."
        );

        match run_phase_session(
            &prompt,
            worktree_path,
            extra_args,
            Some(sid),
            renderer,
            input,
            term_events,
        )
        .await?
        {
            PhaseOutcome::Completed {
                cost, session_id, ..
            } => {
                *total_cost += cost;
                let _ = worktree::clean(worktree_path);

                if worktree::is_rebase_in_progress(worktree_path).unwrap_or(false) {
                    renderer.write_raw("Rebase still in progress — resolution incomplete.\r\n");
                    renderer.write_raw("Aborting rebase, resetting to main.\r\n");
                    worktree::abort_rebase(worktree_path)?;
                    worktree::reset_to_main(worktree_path)?;
                    let _ = worktree::clean(worktree_path);
                    return Ok(false);
                }

                // Resolution complete. Retry the full land — main may have
                // moved while the agent was resolving conflicts.
                renderer.write_raw("Conflict resolution complete, retrying land...\r\n");
                resume_session_id = session_id;
            }
            PhaseOutcome::Exited => {
                let _ = worktree::abort_rebase(worktree_path);
                worktree::reset_to_main(worktree_path)?;
                let _ = worktree::clean(worktree_path);
                return Ok(true);
            }
        }
    }
}

enum PhaseOutcome {
    Completed {
        result_text: String,
        cost: f64,
        session_id: Option<String>,
    },
    Exited,
}

/// Run an interactive claude session for a worker phase (dispatch or agent).
///
/// If `resume` is provided, the session is resumed from the given session ID
/// rather than starting fresh. Used for conflict resolution.
async fn run_phase_session(
    prompt: &str,
    working_dir: &Path,
    extra_args: &[String],
    resume: Option<&str>,
    renderer: &mut Renderer,
    input: &mut InputHandler,
    term_events: &mut EventStream,
) -> Result<PhaseOutcome> {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();

    let session_config = SessionConfig {
        prompt: Some(prompt.to_string()),
        extra_args: extra_args.to_vec(),
        resume: resume.map(String::from),
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
                    session_id: state.session_id.clone(),
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
