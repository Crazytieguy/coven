use std::collections::HashMap;
use std::io::Write;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use crate::agents::{self, AgentDef};
use crate::dispatch::{self, DispatchDecision};
use crate::display::input::{InputAction, InputHandler};
use crate::display::renderer::Renderer;
use crate::fork::{self, ForkConfig};
use crate::session::runner::SessionConfig;
use crate::session::state::SessionState;
use crate::vcr::{Io, IoEvent, VcrContext};
use crate::worker_state;
use crate::worktree::{self, SpawnOptions};

use super::RawModeGuard;
use super::session_loop::{self, SessionOutcome};

/// Shared mutable context threaded through worker phases.
struct PhaseContext<'a, W: Write> {
    renderer: &'a mut Renderer<W>,
    input: &'a mut InputHandler,
    io: &'a mut Io,
    vcr: &'a VcrContext,
    fork_config: Option<&'a ForkConfig>,
}

pub struct WorkerConfig {
    pub show_thinking: bool,
    pub branch: Option<String>,
    pub worktree_base: PathBuf,
    pub extra_args: Vec<String>,
    /// Override for the project root directory (used by test recording).
    pub working_dir: Option<PathBuf>,
    pub fork: bool,
}

/// Serializable args for VCR-recording `worktree::spawn`.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct SpawnArgs {
    repo_path: String,
    branch: Option<String>,
    base_path: String,
}

/// Serializable args for VCR-recording `worker_state::update`.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct WorkerUpdateArgs {
    path: String,
    branch: String,
    agent: Option<String>,
    args: HashMap<String, String>,
}

/// Run a worker: spawn a worktree, loop dispatch → agent → land.
pub async fn worker<W: Write>(
    mut config: WorkerConfig,
    io: &mut Io,
    vcr: &VcrContext,
    writer: W,
) -> Result<()> {
    // Default to acceptEdits (same as other commands) unless the user
    // specified a permission mode. The user is expected to set up persistent
    // permissions for their project so agents can run unattended.
    if !crate::session::runner::has_flag(&config.extra_args, "--permission-mode") {
        config
            .extra_args
            .extend(["--permission-mode".to_string(), "acceptEdits".to_string()]);
    }
    if config.fork {
        config.extra_args.extend(ForkConfig::disallowed_tool_args());
    }

    let configured_dir = config.working_dir.as_ref().map(|d| d.display().to_string());
    let configured_base = config.worktree_base.display().to_string();
    let spawn_args: SpawnArgs = vcr
        .call("worker_paths", (), async |(): &()| {
            let repo_path = match configured_dir {
                Some(s) => s,
                None => std::env::current_dir()?.display().to_string(),
            };
            Ok(SpawnArgs {
                repo_path,
                branch: config.branch.clone(),
                base_path: configured_base,
            })
        })
        .await?;
    let spawn_result = vcr
        .call_typed_err("worktree::spawn", spawn_args, async |a: &SpawnArgs| {
            worktree::spawn(&SpawnOptions {
                repo_path: Path::new(&a.repo_path),
                branch: a.branch.as_deref(),
                base_path: Path::new(&a.base_path),
            })
        })
        .await??;

    let raw = RawModeGuard::acquire(vcr.is_live())?;
    let mut renderer = Renderer::with_writer(writer);
    renderer.set_show_thinking(config.show_thinking);
    renderer.render_help();
    let mut input = InputHandler::new(2);
    let mut total_cost = 0.0;

    let wt_str = spawn_result.worktree_path.display().to_string();

    vcr.call(
        "worker_state::register",
        (wt_str.clone(), spawn_result.branch.clone()),
        async |a: &(String, String)| worker_state::register(Path::new(&a.0), &a.1),
    )
    .await?;

    renderer.set_title(&format!("cv {}", spawn_result.branch));
    renderer.write_raw(&format!(
        "\r\nWorker started: {} ({})\r\n",
        spawn_result.branch,
        spawn_result.worktree_path.display()
    ));

    let fork_config = ForkConfig::if_enabled(
        config.fork,
        &config.extra_args,
        &Some(spawn_result.worktree_path.clone()),
    );

    let mut ctx = PhaseContext {
        renderer: &mut renderer,
        input: &mut input,
        io,
        vcr,
        fork_config: fork_config.as_ref(),
    };

    let result = worker_loop(
        &config,
        &spawn_result.worktree_path,
        &spawn_result.branch,
        &mut ctx,
        &mut total_cost,
    )
    .await;

    drop(raw);
    renderer.set_title("");

    vcr.call(
        "worker_state::deregister",
        (wt_str.clone(), spawn_result.branch.clone()),
        async |a: &(String, String)| -> Result<()> {
            worker_state::deregister(Path::new(&a.0), &a.1);
            Ok(())
        },
    )
    .await?;

    renderer.write_raw("\r\nRemoving worktree...\r\n");
    if let Err(e) = vcr
        .call_typed_err("worktree::remove", wt_str, async |p: &String| {
            worktree::remove(Path::new(p))
        })
        .await?
    {
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
async fn run_dispatch<W: Write>(
    worktree_path: &Path,
    branch: &str,
    extra_args: &[String],
    worker_status: &str,
    ctx: &mut PhaseContext<'_, W>,
) -> Result<Option<DispatchResult>> {
    ctx.renderer
        .set_title(&format!("cv dispatch \u{2014} {branch}"));
    ctx.renderer.write_raw("\r\n=== Dispatch ===\r\n\r\n");

    let agents_dir = worktree_path.join(agents::AGENTS_DIR);
    let agents_dir_str = agents_dir.display().to_string();
    let agent_defs = ctx
        .vcr
        .call("agents::load_agents", agents_dir_str, async |d: &String| {
            agents::load_agents(Path::new(d))
        })
        .await?;
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
    } = run_phase_session(&dispatch_prompt, worktree_path, extra_args, None, ctx).await?
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

            ctx.renderer.write_raw(&format!(
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
                ctx,
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

async fn worker_loop<W: Write>(
    config: &WorkerConfig,
    worktree_path: &Path,
    branch: &str,
    ctx: &mut PhaseContext<'_, W>,
    total_cost: &mut f64,
) -> Result<()> {
    loop {
        // Sync worktree to latest main so dispatch sees current issue state
        let wt_str = worktree_path.display().to_string();
        ctx.vcr
            .call_typed_err(
                "worktree::sync_to_main",
                wt_str.clone(),
                async |p: &String| worktree::sync_to_main(Path::new(p)),
            )
            .await?
            .context("failed to sync worktree to main")?;

        // === Phase 1: Dispatch (under lock) ===
        let lock = ctx
            .vcr
            .call(
                "worker_state::acquire_dispatch_lock",
                wt_str.clone(),
                async |p: &String| worker_state::acquire_dispatch_lock(Path::new(p)).await,
            )
            .await?;
        let all_workers = ctx
            .vcr
            .call(
                "worker_state::read_all",
                wt_str.clone(),
                async |p: &String| worker_state::read_all(Path::new(p)),
            )
            .await?;
        let worker_status = worker_state::format_status(&all_workers, branch);

        let Some(dispatch) = run_dispatch(
            worktree_path,
            branch,
            &config.extra_args,
            &worker_status,
            ctx,
        )
        .await?
        else {
            return Ok(());
        };

        // Update worker state before releasing lock so the next dispatch sees it
        let empty = HashMap::new();
        let (agent_name, agent_args) = match &dispatch.decision {
            DispatchDecision::Sleep => (None, &empty),
            DispatchDecision::RunAgent { agent, args } => (Some(agent.as_str()), args),
        };
        vcr_update_worker_state(ctx.vcr, &wt_str, branch, agent_name, agent_args).await?;
        drop(lock);

        *total_cost += dispatch.cost;
        ctx.renderer
            .write_raw(&format!("  Total cost: ${total_cost:.2}\r\n"));

        match dispatch.decision {
            DispatchDecision::Sleep => {
                ctx.renderer
                    .set_title(&format!("cv sleeping \u{2014} {branch}"));
                ctx.renderer
                    .write_raw("\r\nDispatch: sleep — waiting for new commits...\r\n");
                ctx.io.clear_event_channel();
                let wait =
                    wait_for_new_commits(worktree_path, ctx.renderer, ctx.input, ctx.io, ctx.vcr);
                if matches!(wait.await?, WaitOutcome::Exited) {
                    return Ok(());
                }
            }
            DispatchDecision::RunAgent { agent, args } => {
                let args_display = args
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                ctx.renderer
                    .write_raw(&format!("\r\nDispatch: {agent} {args_display}\r\n"));

                let agent_def = dispatch
                    .agent_defs
                    .iter()
                    .find(|a| a.name == agent)
                    .with_context(|| format!("dispatch chose unknown agent: {agent}"))?;

                let agent_prompt = agent_def.render(&args)?;
                ctx.renderer
                    .write_raw(&format!("\r\n=== Agent: {agent} ===\r\n\r\n"));
                let title_suffix = if args_display.is_empty() {
                    agent
                } else {
                    format!("{agent} {args_display}")
                };
                ctx.renderer
                    .set_title(&format!("cv {title_suffix} \u{2014} {branch}"));

                let should_exit = run_agent(
                    &agent_prompt,
                    worktree_path,
                    &config.extra_args,
                    ctx,
                    total_cost,
                )
                .await?;
                if should_exit {
                    return Ok(());
                }

                // Clear state so other dispatchers don't see stale agent info
                vcr_update_worker_state(ctx.vcr, &wt_str, branch, None, &HashMap::new()).await?;
            }
        }
    }
}

/// Run the agent phase: execute the agent session, ensure commits, and land.
/// Returns true if the worker should exit (user interrupted).
async fn run_agent<W: Write>(
    prompt: &str,
    worktree_path: &Path,
    extra_args: &[String],
    ctx: &mut PhaseContext<'_, W>,
    total_cost: &mut f64,
) -> Result<bool> {
    let agent_session_id =
        match run_phase_session(prompt, worktree_path, extra_args, None, ctx).await? {
            PhaseOutcome::Completed {
                cost, session_id, ..
            } => {
                *total_cost += cost;
                ctx.renderer
                    .write_raw(&format!("  Total cost: ${total_cost:.2}\r\n"));
                session_id
            }
            PhaseOutcome::Exited => return Ok(true),
        };

    // === Phase 3: Land ===
    // Clean untracked files before landing. Agents should commit
    // their work; leftover files (test artifacts, temp files) must
    // not block landing and cause committed work to be discarded.
    warn_clean(worktree_path, ctx.renderer, ctx.vcr).await?;

    let commit_result =
        ensure_commits(worktree_path, agent_session_id, extra_args, ctx, total_cost).await?;

    match commit_result {
        CommitCheck::HasCommits { session_id } => {
            let should_exit = land_or_resolve(
                worktree_path,
                session_id.as_deref(),
                extra_args,
                ctx,
                total_cost,
            )
            .await?;
            if should_exit {
                return Ok(true);
            }
        }
        CommitCheck::NoCommits => {
            ctx.renderer
                .write_raw("Agent produced no commits — skipping land.\r\n");
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
async fn ensure_commits<W: Write>(
    worktree_path: &Path,
    agent_session_id: Option<String>,
    extra_args: &[String],
    ctx: &mut PhaseContext<'_, W>,
    total_cost: &mut f64,
) -> Result<CommitCheck> {
    let wt_str = worktree_path.display().to_string();
    if vcr_has_unique_commits(ctx.vcr, wt_str.clone()).await?? {
        return Ok(CommitCheck::HasCommits {
            session_id: agent_session_id,
        });
    }

    let Some(sid) = agent_session_id.as_deref() else {
        ctx.renderer
            .write_raw("Agent produced no commits and no session to resume.\r\n");
        return Ok(CommitCheck::NoCommits);
    };

    ctx.renderer
        .write_raw("Agent produced no commits — resuming session to ask for a commit.\r\n\r\n");

    match run_phase_session(
        "You finished without committing anything. \
         If you have changes worth keeping, please commit them now. \
         If there's nothing to commit, just confirm that.",
        worktree_path,
        extra_args,
        Some(sid),
        ctx,
    )
    .await?
    {
        PhaseOutcome::Completed {
            cost, session_id, ..
        } => {
            *total_cost += cost;
            ctx.renderer
                .write_raw(&format!("  Total cost: ${total_cost:.2}\r\n"));
            warn_clean(worktree_path, ctx.renderer, ctx.vcr).await?;

            if vcr_has_unique_commits(ctx.vcr, wt_str).await?? {
                Ok(CommitCheck::HasCommits { session_id })
            } else {
                Ok(CommitCheck::NoCommits)
            }
        }
        PhaseOutcome::Exited => Ok(CommitCheck::Exited),
    }
}

/// Maximum land attempts (shared across ff-retry and conflict resolution)
/// before pausing for user input.
const MAX_LAND_ATTEMPTS: u32 = 5;

/// Result of a single `worktree::land` call, flattened for easy matching.
enum LandAttempt {
    Landed { branch: String, main_branch: String },
    Conflict(Vec<String>),
    FastForwardRace,
    OtherError(anyhow::Error),
}

/// Call `worktree::land` via VCR and map the result into a flat enum.
async fn try_land(vcr: &VcrContext, wt_str: String) -> Result<LandAttempt> {
    match vcr
        .call_typed_err("worktree::land", wt_str, async |p: &String| {
            worktree::land(Path::new(p))
        })
        .await?
    {
        Ok(result) => Ok(LandAttempt::Landed {
            branch: result.branch,
            main_branch: result.main_branch,
        }),
        Err(worktree::WorktreeError::RebaseConflict(files)) => Ok(LandAttempt::Conflict(files)),
        Err(worktree::WorktreeError::FastForwardFailed) => Ok(LandAttempt::FastForwardRace),
        Err(e) => Ok(LandAttempt::OtherError(e.into())),
    }
}

/// Handle a fast-forward race: bump attempts, pause if too many.
/// Returns `Break(true)` to exit, `Continue(())` to retry.
async fn handle_ff_retry<W: Write>(
    attempts: &mut u32,
    ctx: &mut PhaseContext<'_, W>,
) -> Result<ControlFlow<bool>> {
    *attempts += 1;
    if *attempts > MAX_LAND_ATTEMPTS {
        ctx.renderer.write_raw(&format!(
            "Fast-forward failed after {MAX_LAND_ATTEMPTS} attempts \
             — pausing worker. Press Enter to retry.\r\n",
        ));
        if wait_for_enter_or_exit(ctx.io).await? {
            return Ok(ControlFlow::Break(true));
        }
        *attempts = 0;
    } else {
        ctx.renderer
            .write_raw("Main advanced during land — retrying...\r\n");
    }
    Ok(ControlFlow::Continue(()))
}

/// Handle a non-conflict, non-ff error: abort rebase and pause for user.
/// No attempts counter — user manually presses Enter each time,
/// so they can inspect the worktree and fix the issue before retrying.
/// Returns `Break(true)` to exit, `Continue(())` to retry.
async fn handle_land_error<W: Write>(
    err: anyhow::Error,
    wt_str: &str,
    ctx: &mut PhaseContext<'_, W>,
) -> Result<ControlFlow<bool>> {
    let _ = vcr_abort_rebase(ctx.vcr, wt_str.to_string()).await?;
    ctx.renderer.write_raw(&format!(
        "Land failed: {err} — pausing worker. Press Enter to retry.\r\n",
    ));
    if wait_for_enter_or_exit(ctx.io).await? {
        return Ok(ControlFlow::Break(true));
    }
    Ok(ControlFlow::Continue(()))
}

/// Handle a rebase conflict: bump attempts, resolve via agent session.
/// Returns `Break(true)` to exit, `Continue(())` to retry landing.
async fn handle_conflict<W: Write>(
    conflict_files: Vec<String>,
    attempts: &mut u32,
    resume_session_id: &mut Option<String>,
    worktree_path: &Path,
    extra_args: &[String],
    ctx: &mut PhaseContext<'_, W>,
    total_cost: &mut f64,
) -> Result<ControlFlow<bool>> {
    let wt_str = worktree_path.display().to_string();
    *attempts += 1;

    if *attempts > MAX_LAND_ATTEMPTS {
        vcr_abort_rebase(ctx.vcr, wt_str.clone()).await??;
        ctx.renderer.write_raw(&format!(
            "Conflict resolution failed after {MAX_LAND_ATTEMPTS} attempts \
             — pausing worker. Press Enter to retry.\r\n",
        ));
        if wait_for_enter_or_exit(ctx.io).await? {
            return Ok(ControlFlow::Break(true));
        }
        *attempts = 0;
        return Ok(ControlFlow::Continue(()));
    }

    let files_display = conflict_files.join(", ");

    let Some(sid) = resume_session_id.as_deref() else {
        vcr_abort_rebase(ctx.vcr, wt_str.clone()).await??;
        bail!(
            "Rebase conflict in {files_display} but no session ID available \
             — this should be impossible"
        );
    };

    ctx.renderer.write_raw(&format!(
        "Rebase conflict in: {files_display} — resuming session to resolve.\r\n"
    ));
    ctx.renderer
        .write_raw("\r\n=== Conflict Resolution ===\r\n\r\n");

    let prompt = format!(
        "The rebase onto main hit conflicts in: {files_display}\n\n\
         Resolve the conflicts in those files, stage them with `git add`, \
         and run `git rebase --continue`. If more conflicts appear after \
         continuing, resolve those too until the rebase completes."
    );

    match resolve_conflict(&prompt, worktree_path, sid, extra_args, ctx).await? {
        ResolveOutcome::Resolved { session_id, cost } => {
            *total_cost += cost;
            ctx.renderer
                .write_raw("Conflict resolution complete, retrying land...\r\n");
            *resume_session_id = session_id;
        }
        ResolveOutcome::Incomplete { session_id, cost } => {
            *total_cost += cost;
            ctx.renderer.write_raw("Retrying land...\r\n");
            *resume_session_id = session_id;
        }
        ResolveOutcome::Exited => return Ok(ControlFlow::Break(true)),
    }
    Ok(ControlFlow::Continue(()))
}

/// Attempt to land and, on rebase conflict, resume the agent session to resolve.
///
/// After successful conflict resolution, retries the full land (rebase + ff-merge)
/// rather than just ff-merge. This handles the case where another worker landed
/// while conflict resolution was in progress, which would cause a bare ff-merge
/// to fail and silently lose the resolved work.
///
/// Returns true if the worker should exit (user interrupted during resolution).
async fn land_or_resolve<W: Write>(
    worktree_path: &Path,
    session_id: Option<&str>,
    extra_args: &[String],
    ctx: &mut PhaseContext<'_, W>,
    total_cost: &mut f64,
) -> Result<bool> {
    ctx.renderer.write_raw("\r\n=== Landing ===\r\n");

    // Track the session to resume for conflict resolution. Starts as the
    // agent's session, then updated to the resolution session's ID so
    // subsequent rounds of conflicts can be resolved in-context.
    let mut resume_session_id = session_id.map(String::from);
    let mut attempts: u32 = 0;
    let wt_str = worktree_path.display().to_string();

    loop {
        match try_land(ctx.vcr, wt_str.clone()).await? {
            LandAttempt::Landed {
                branch,
                main_branch,
            } => {
                ctx.renderer
                    .write_raw(&format!("Landed {branch} onto {main_branch}\r\n"));
                return Ok(false);
            }
            LandAttempt::FastForwardRace => {
                if let ControlFlow::Break(exit) = handle_ff_retry(&mut attempts, ctx).await? {
                    return Ok(exit);
                }
            }
            LandAttempt::OtherError(err) => {
                if let ControlFlow::Break(exit) = handle_land_error(err, &wt_str, ctx).await? {
                    return Ok(exit);
                }
            }
            LandAttempt::Conflict(files) => {
                if let ControlFlow::Break(exit) = handle_conflict(
                    files,
                    &mut attempts,
                    &mut resume_session_id,
                    worktree_path,
                    extra_args,
                    ctx,
                    total_cost,
                )
                .await?
                {
                    return Ok(exit);
                }
            }
        }
    }
}

enum ResolveOutcome {
    /// Conflict resolved (possibly after nudge), retry land with this session ID.
    Resolved {
        session_id: Option<String>,
        cost: f64,
    },
    /// Rebase still incomplete after nudge — rebase aborted, retry loop continues.
    Incomplete {
        session_id: Option<String>,
        cost: f64,
    },
    /// User exited — cleanup already done.
    Exited,
}

/// Run a conflict resolution session, nudging once if the rebase remains incomplete.
async fn resolve_conflict<W: Write>(
    prompt: &str,
    worktree_path: &Path,
    sid: &str,
    extra_args: &[String],
    ctx: &mut PhaseContext<'_, W>,
) -> Result<ResolveOutcome> {
    let wt_str = worktree_path.display().to_string();

    let PhaseOutcome::Completed {
        cost, session_id, ..
    } = run_phase_session(prompt, worktree_path, extra_args, Some(sid), ctx).await?
    else {
        abort_and_reset(worktree_path, ctx.renderer, ctx.vcr).await?;
        return Ok(ResolveOutcome::Exited);
    };

    warn_clean(worktree_path, ctx.renderer, ctx.vcr).await?;

    let is_rebasing = vcr_is_rebase_in_progress(ctx.vcr, wt_str.clone())
        .await?
        .unwrap_or(false);
    if !is_rebasing {
        if session_id.as_deref() != Some(sid) {
            ctx.renderer.write_raw(
                "Warning: resolution session returned a different session ID than expected.\r\n",
            );
        }
        return Ok(ResolveOutcome::Resolved { session_id, cost });
    }

    // Nudge Claude to complete the rebase
    ctx.renderer
        .write_raw("Rebase still in progress — nudging session to complete it.\r\n\r\n");
    let nudge_sid = session_id.as_deref().unwrap_or(sid);

    let PhaseOutcome::Completed {
        cost: nudge_cost,
        session_id: nudge_session_id,
        ..
    } = run_phase_session(
        "The rebase is still in progress — please run `git rebase --continue` to complete it.",
        worktree_path,
        extra_args,
        Some(nudge_sid),
        ctx,
    )
    .await?
    else {
        abort_and_reset(worktree_path, ctx.renderer, ctx.vcr).await?;
        return Ok(ResolveOutcome::Exited);
    };

    let total_cost = cost + nudge_cost;
    warn_clean(worktree_path, ctx.renderer, ctx.vcr).await?;

    let is_rebasing = vcr_is_rebase_in_progress(ctx.vcr, wt_str.clone())
        .await?
        .unwrap_or(false);
    if is_rebasing {
        ctx.renderer
            .write_raw("Rebase still in progress after nudge — aborting this attempt.\r\n");
        vcr_abort_rebase(ctx.vcr, wt_str).await??;
        return Ok(ResolveOutcome::Incomplete {
            session_id: nudge_session_id,
            cost: total_cost,
        });
    }

    Ok(ResolveOutcome::Resolved {
        session_id: nudge_session_id,
        cost: total_cost,
    })
}

/// Wait for Enter (returns false) or Ctrl-C/Ctrl-D/stream end (returns true = should exit).
async fn wait_for_enter_or_exit(io: &mut Io) -> Result<bool> {
    loop {
        let io_event = io.next_event().await?;
        if let IoEvent::Terminal(Event::Key(key_event)) = io_event {
            match key_event.code {
                KeyCode::Enter => return Ok(false),
                KeyCode::Char('c' | 'd') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(true);
                }
                _ => {}
            }
        }
    }
}

/// Abort any in-progress rebase, reset to main, and clean the worktree.
async fn abort_and_reset<W: Write>(
    worktree_path: &Path,
    renderer: &mut Renderer<W>,
    vcr: &VcrContext,
) -> Result<()> {
    let wt_str = worktree_path.display().to_string();
    let _ = vcr_abort_rebase(vcr, wt_str.clone()).await?;
    vcr.call_typed_err("worktree::reset_to_main", wt_str, async |p: &String| {
        worktree::reset_to_main(Path::new(p))
    })
    .await??;
    warn_clean(worktree_path, renderer, vcr).await?;
    Ok(())
}

/// Run `git clean -fd` and warn (but don't fail) if it errors.
async fn warn_clean<W: Write>(
    worktree_path: &Path,
    renderer: &mut Renderer<W>,
    vcr: &VcrContext,
) -> Result<()> {
    let wt_str = worktree_path.display().to_string();
    if let Err(e) = vcr
        .call_typed_err("worktree::clean", wt_str, async |p: &String| {
            worktree::clean(Path::new(p))
        })
        .await?
    {
        renderer.write_raw(&format!("Warning: worktree clean failed: {e}\r\n"));
    }
    Ok(())
}

/// VCR-wrapped `worktree::abort_rebase`.
async fn vcr_abort_rebase(
    vcr: &VcrContext,
    wt_str: String,
) -> Result<Result<(), worktree::WorktreeError>> {
    vcr.call_typed_err("worktree::abort_rebase", wt_str, async |p: &String| {
        worktree::abort_rebase(Path::new(p))
    })
    .await
}

/// VCR-wrapped `worktree::has_unique_commits`.
async fn vcr_has_unique_commits(
    vcr: &VcrContext,
    wt_str: String,
) -> Result<Result<bool, worktree::WorktreeError>> {
    vcr.call_typed_err(
        "worktree::has_unique_commits",
        wt_str,
        async |p: &String| worktree::has_unique_commits(Path::new(p)),
    )
    .await
}

/// VCR-wrapped `worktree::is_rebase_in_progress`.
async fn vcr_is_rebase_in_progress(
    vcr: &VcrContext,
    wt_str: String,
) -> Result<Result<bool, worktree::WorktreeError>> {
    vcr.call_typed_err(
        "worktree::is_rebase_in_progress",
        wt_str,
        async |p: &String| worktree::is_rebase_in_progress(Path::new(p)),
    )
    .await
}

/// VCR-wrapped `main_head_sha`.
async fn vcr_main_head_sha(vcr: &VcrContext, wt_str: String) -> Result<String> {
    vcr.call("main_head_sha", wt_str, async |p: &String| {
        main_head_sha(Path::new(p))
    })
    .await
}

/// VCR-wrapped `worker_state::update`.
async fn vcr_update_worker_state(
    vcr: &VcrContext,
    path: &str,
    branch: &str,
    agent: Option<&str>,
    args: &HashMap<String, String>,
) -> Result<()> {
    vcr.call(
        "worker_state::update",
        WorkerUpdateArgs {
            path: path.to_string(),
            branch: branch.to_string(),
            agent: agent.map(String::from),
            args: args.clone(),
        },
        async |a: &WorkerUpdateArgs| {
            worker_state::update(Path::new(&a.path), &a.branch, a.agent.as_deref(), &a.args)
        },
    )
    .await
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
async fn run_phase_session<W: Write>(
    prompt: &str,
    working_dir: &Path,
    extra_args: &[String],
    resume: Option<&str>,
    ctx: &mut PhaseContext<'_, W>,
) -> Result<PhaseOutcome> {
    let append_system_prompt = ctx
        .fork_config
        .map(|_| fork::fork_system_prompt().to_string());
    let session_config = SessionConfig {
        prompt: Some(prompt.to_string()),
        extra_args: extra_args.to_vec(),
        append_system_prompt,
        resume: resume.map(String::from),
        working_dir: Some(working_dir.to_path_buf()),
    };

    let mut runner = session_loop::spawn_session(session_config.clone(), ctx.io, ctx.vcr).await?;
    let mut state = SessionState::default();

    loop {
        let outcome = session_loop::run_session(
            &mut runner,
            &mut state,
            ctx.renderer,
            ctx.input,
            ctx.io,
            ctx.vcr,
            ctx.fork_config,
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
                ctx.io.clear_event_channel();
                let Some(session_id) = state.session_id.take() else {
                    return Ok(PhaseOutcome::Exited);
                };
                ctx.renderer.render_interrupted();

                match session_loop::wait_for_user_input(ctx.input, ctx.renderer, ctx.io, ctx.vcr)
                    .await?
                {
                    Some(text) => {
                        let resume_config = session_config.resume_with(text, session_id);
                        runner =
                            session_loop::spawn_session(resume_config, ctx.io, ctx.vcr).await?;
                        let prev_session_id = state.session_id.clone();
                        state = SessionState::default();
                        state.session_id = prev_session_id;
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

/// Set up a filesystem watcher on the git refs for the main branch.
///
/// Watches `<git-common-dir>/refs/heads/<main-branch>` (loose ref) and
/// `<git-common-dir>/packed-refs` (updated during gc). Returns the watcher
/// (must be kept alive) and a receiver that fires on any ref change.
///
/// Best-effort: if the git paths can't be resolved (e.g. during VCR replay
/// with a dummy worktree), returns a watcher that watches nothing. The
/// receiver will never fire, which is fine — the VCR-replayed `next_event`
/// branch always wins the select in that case.
fn setup_ref_watcher(
    worktree_path: &Path,
) -> Result<(notify::RecommendedWatcher, tokio::sync::mpsc::Receiver<()>)> {
    let (tx, rx) = tokio::sync::mpsc::channel(1);

    let mut watcher = notify::recommended_watcher(move |_: notify::Result<notify::Event>| {
        // Best-effort send; if the channel is full, a notification is already pending.
        let _ = tx.try_send(());
    })
    .context("failed to create filesystem watcher")?;

    // Best-effort: resolve git paths and set up watches. If any step fails
    // (e.g. worktree doesn't exist during VCR replay), skip watching.
    if let Some(paths) = resolve_ref_paths(worktree_path) {
        if paths.refs_heads_dir.exists() {
            let _ = watcher.watch(&paths.refs_heads_dir, RecursiveMode::Recursive);
        } else if paths.loose_ref.exists() {
            let _ = watcher.watch(&paths.loose_ref, RecursiveMode::NonRecursive);
        }
        if paths.packed_refs.exists() {
            let _ = watcher.watch(&paths.packed_refs, RecursiveMode::NonRecursive);
        }
    }

    Ok((watcher, rx))
}

struct RefPaths {
    refs_heads_dir: PathBuf,
    loose_ref: PathBuf,
    packed_refs: PathBuf,
}

/// Resolve the git ref paths to watch. Returns `None` if the git commands fail.
fn resolve_ref_paths(worktree_path: &Path) -> Option<RefPaths> {
    let main_branch = worktree::main_branch_name(worktree_path).ok()?;

    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree_path)
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let git_common_dir = if Path::new(&raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        worktree_path.join(raw)
    };

    Some(RefPaths {
        refs_heads_dir: git_common_dir.join("refs/heads"),
        loose_ref: git_common_dir.join("refs/heads").join(&main_branch),
        packed_refs: git_common_dir.join("packed-refs"),
    })
}

/// Wait for new commits on main using filesystem notifications, while allowing
/// the user to exit.
async fn wait_for_new_commits<W: Write>(
    worktree_path: &Path,
    renderer: &mut Renderer<W>,
    input: &mut InputHandler,
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<WaitOutcome> {
    let wt_str = worktree_path.display().to_string();
    let initial_head = vcr_main_head_sha(vcr, wt_str.clone()).await?;

    // _watcher must stay alive for the duration of the loop.
    let (_watcher, mut rx) = setup_ref_watcher(worktree_path)?;

    loop {
        tokio::select! {
            _ = rx.recv() => {
                let current = vcr_main_head_sha(vcr, wt_str.clone()).await?;
                if current != initial_head {
                    renderer.write_raw("New commits detected on main.\r\n");
                    return Ok(WaitOutcome::NewCommits);
                }
                // Spurious notification — loop and wait again
            }
            event = vcr.call("next_event", (), async |(): &()| io.next_event().await) => {
                let event = event?;
                if let IoEvent::Terminal(Event::Key(key_event)) = event {
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
    let main_branch = worktree::main_branch_name(worktree_path)?;

    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree_path)
        .args(["rev-parse", &main_branch])
        .output()
        .context("failed to run git rev-parse")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git rev-parse failed: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
