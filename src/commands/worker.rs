use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use crossterm::event::Event;
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use crate::agents::{self, AgentDef};
use crate::config;
use crate::display::input::{InputAction, InputHandler};
use crate::display::renderer::Renderer;
use crate::fork::{self, ForkConfig};
use crate::semaphore;
use crate::session::runner::SessionConfig;
use crate::session::state::SessionState;
use crate::transition::{self, Transition};
use crate::vcr::{Io, IoEvent, VcrContext};
use crate::worker_state;
use crate::worktree::{self, SpawnOptions};

use crate::session::event_loop::{self, SessionOutcome};

use super::{RawModeGuard, setup_display};

/// Shared mutable context threaded through worker phases.
struct PhaseContext<'a, W: Write> {
    renderer: &'a mut Renderer<W>,
    input: &'a mut InputHandler,
    io: &'a mut Io,
    vcr: &'a VcrContext,
    fork_config: Option<&'a ForkConfig>,
    total_cost: f64,
}

pub struct WorkerConfig {
    pub show_thinking: bool,
    pub branch: Option<String>,
    pub worktree_base: PathBuf,
    pub extra_args: Vec<String>,
    /// Override for the project root directory (used by test recording).
    pub working_dir: Option<PathBuf>,
    pub fork: bool,
    /// Override terminal width for display truncation (used in tests).
    pub term_width: Option<usize>,
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

/// Serializable args for VCR-recording `semaphore::acquire`.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct SemaphoreAcquireArgs {
    path: String,
    agent: String,
    max_concurrency: u32,
}

/// Run a worker: spawn a worktree, loop through the generic agent loop.
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
    let (mut renderer, mut input) = setup_display(writer, config.term_width, config.show_thinking);
    renderer.render_help();

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
        total_cost: 0.0,
    };

    let result = worker_loop(
        &config,
        &spawn_result.worktree_path,
        &spawn_result.branch,
        &mut ctx,
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
        .call_typed_err("worktree::remove", wt_str.clone(), async |p: &String| {
            worktree::remove(Path::new(p), false)
        })
        .await?
    {
        renderer.write_raw(&format!(
            "Warning: failed to remove worktree: {e}\r\n\
             hint: git worktree remove --force {wt_str}\r\n"
        ));
    }

    result
}

/// Generic agent loop: entry agent → parse transition → next agent → ...
///
/// Outer loop: sync to main, run entry agent.
/// Inner loop: chain agents via `<next>` transitions.
async fn worker_loop<W: Write>(
    config: &WorkerConfig,
    worktree_path: &Path,
    branch: &str,
    ctx: &mut PhaseContext<'_, W>,
) -> Result<()> {
    let wt_str = worktree_path.display().to_string();

    let project_config: config::Config = ctx
        .vcr
        .call("config::load", wt_str.clone(), async |p: &String| {
            config::load(Path::new(p))
        })
        .await?;

    loop {
        // Sync worktree to latest main so the entry agent sees current state
        ctx.vcr
            .call_typed_err(
                "worktree::sync_to_main",
                wt_str.clone(),
                async |p: &String| worktree::sync_to_main(Path::new(p)),
            )
            .await?
            .context("failed to sync worktree to main")?;

        let chain_result = run_agent_chain(
            config,
            worktree_path,
            branch,
            &project_config.entry_agent,
            ctx,
        )
        .await?;

        match chain_result {
            ChainResult::Sleep => {
                ctx.renderer
                    .set_title(&format!("cv sleeping \u{2014} {branch}"));
                ctx.renderer
                    .write_raw("\r\nTransition: sleep \u{2014} waiting for new commits...\r\n");
                ctx.io.clear_event_channel();
                let wait =
                    wait_for_new_commits(worktree_path, ctx.renderer, ctx.input, ctx.io, ctx.vcr);
                if matches!(wait.await?, WaitOutcome::Exited) {
                    return Ok(());
                }
            }
            ChainResult::Exited => return Ok(()),
        }
    }
}

/// Result of running an agent chain.
enum ChainResult {
    /// Chain ended with a sleep transition — wait for new commits.
    Sleep,
    /// User exited.
    Exited,
}

/// Run a chain of agents starting from `entry_agent`, following `<next>` transitions.
async fn run_agent_chain<W: Write>(
    config: &WorkerConfig,
    worktree_path: &Path,
    branch: &str,
    entry_agent: &str,
    ctx: &mut PhaseContext<'_, W>,
) -> Result<ChainResult> {
    let wt_str = worktree_path.display().to_string();
    let mut agent_name = entry_agent.to_string();
    let mut agent_args: HashMap<String, String> = HashMap::new();

    let system_doc = vcr_load_system_doc(ctx.vcr, &wt_str).await?;
    let main_worktree_branch = vcr_main_branch_name(ctx.vcr, &wt_str).await?;

    loop {
        let agent_defs = vcr_load_agents(ctx.vcr, worktree_path).await?;

        let agent_def = agent_defs
            .iter()
            .find(|a| a.name == agent_name)
            .with_context(|| format!("unknown agent: {agent_name}"))?;

        let _semaphore_permit =
            vcr_acquire_semaphore(ctx.vcr, &wt_str, &agent_name, agent_def).await?;

        vcr_update_worker_state(ctx.vcr, &wt_str, branch, Some(&agent_name), &agent_args).await?;

        let agent_prompt = agent_def.render(&agent_args)?;

        // Merge per-agent claude_args with worker-level extra_args.
        // Agent args come first, worker args last (CLI-level `-- [ARGS]` can override).
        let mut merged_args = agent_def.frontmatter.claude_args.clone();
        merged_args.extend(config.extra_args.iter().cloned());

        let transition_prompt = transition::format_transition_system_prompt(&agent_defs);
        let all_workers = ctx
            .vcr
            .call(
                "worker_state::read_all",
                wt_str.clone(),
                async |p: &String| worker_state::read_all(Path::new(p)),
            )
            .await?;
        let others: Vec<_> = all_workers.iter().filter(|s| s.branch != branch).collect();
        let worker_status_section = if others.is_empty() {
            "\n\nNo other workers active.".to_string()
        } else {
            format!(
                "\n\n## Worker Status\n\n{}",
                worker_state::format_workers(&others, worker_state::StatusStyle::Dispatch)
            )
        };
        let system_prompt = build_system_prompt(
            &system_doc,
            &transition_prompt,
            &worker_status_section,
            &main_worktree_branch,
            ctx.fork_config,
        );

        ctx.renderer
            .write_raw(&format!("\r\n=== Agent: {agent_name} ===\r\n\r\n"));
        let title_suffix = match agent_def.render_title(&agent_args)? {
            Some(t) => format!("{agent_name}: {t}"),
            None => match format_args_display(&agent_args) {
                d if d.is_empty() => agent_name.clone(),
                d => format!("{agent_name} {d}"),
            },
        };
        ctx.renderer
            .set_title(&format!("cv {title_suffix} \u{2014} {branch}"));

        let parsed_transition = run_phase_with_wait(
            &agent_prompt,
            worktree_path,
            &merged_args,
            &system_prompt,
            &agent_defs,
            ctx,
        )
        .await?;
        let Some(parsed_transition) = parsed_transition else {
            return Ok(ChainResult::Exited);
        };

        match parsed_transition {
            Transition::Next { agent, args } => {
                let args_display = format_args_display(&args);
                ctx.renderer
                    .write_raw(&format!("\r\nTransition: {agent} {args_display}\r\n"));
                agent_name = agent;
                agent_args = args;
            }
            Transition::Sleep => {
                vcr_update_worker_state(ctx.vcr, &wt_str, branch, None, &HashMap::new()).await?;
                return Ok(ChainResult::Sleep);
            }
            Transition::WaitForUser { .. } => {
                bail!("unexpected WaitForUser transition in agent chain")
            }
        }
    }
}

/// Run a phase session, looping on `WaitForUser` transitions.
///
/// If the agent outputs `<wait-for-user>`, we wait for user input, resume the
/// session, and repeat until we get a `Next` or `Sleep` transition.
/// Returns `None` if the user exited.
async fn run_phase_with_wait<W: Write>(
    initial_prompt: &str,
    worktree_path: &Path,
    extra_args: &[String],
    system_prompt: &str,
    agents: &[AgentDef],
    ctx: &mut PhaseContext<'_, W>,
) -> Result<Option<Transition>> {
    let mut phase_prompt = initial_prompt.to_string();
    let mut phase_resume: Option<String> = None;

    loop {
        let PhaseOutcome::Completed {
            result_text,
            cost,
            session_id,
        } = run_phase_session(
            &phase_prompt,
            worktree_path,
            extra_args,
            phase_resume.as_deref(),
            Some(system_prompt),
            ctx,
        )
        .await?
        else {
            return Ok(None);
        };

        ctx.total_cost += cost;
        ctx.renderer
            .write_raw(&format!("  Total cost: ${:.2}\r\n", ctx.total_cost));

        let Some(transition) = parse_transition_with_retry(
            &result_text,
            session_id.as_deref(),
            worktree_path,
            extra_args,
            Some(system_prompt),
            agents,
            ctx,
        )
        .await?
        else {
            return Ok(None);
        };

        match transition {
            Transition::WaitForUser { reason } => {
                ctx.renderer.write_raw("\x07");
                ctx.renderer
                    .write_raw(&format!("\r\nWaiting for user: {reason}\r\n"));
                let sid = session_id
                    .as_deref()
                    .context("no session ID for wait-for-user resume")?;
                let Some(user_text) = event_loop::wait_for_interrupt_input(
                    ctx.input,
                    ctx.renderer,
                    ctx.io,
                    ctx.vcr,
                    sid,
                    Some(worktree_path),
                    extra_args,
                )
                .await?
                else {
                    return Ok(None);
                };
                phase_prompt = user_text;
                phase_resume = session_id;
            }
            other => return Ok(Some(other)),
        }
    }
}

/// Assemble the system prompt from its components.
fn build_system_prompt(
    system_doc: &str,
    transition_prompt: &str,
    worker_status_section: &str,
    main_worktree_branch: &str,
    fork_config: Option<&ForkConfig>,
) -> String {
    let mut prompt = String::new();
    if !system_doc.is_empty() {
        prompt.push_str(system_doc);
        prompt.push_str("\n\n");
    }
    prompt.push_str(transition_prompt);
    prompt.push_str(worker_status_section);
    let _ = write!(prompt, "\n\nMain worktree branch: {main_worktree_branch}");
    if fork_config.is_some() {
        prompt.push_str("\n\n");
        prompt.push_str(fork::fork_system_prompt());
    }
    prompt
}

/// Format args as a sorted display string.
fn format_args_display(args: &HashMap<String, String>) -> String {
    let mut parts: Vec<_> = args.iter().map(|(k, v)| format!("{k}={v}")).collect();
    parts.sort();
    parts.join(" ")
}

/// Maximum number of automatic corrective retries before falling back to user input.
const MAX_TRANSITION_RETRIES: usize = 3;

/// Parse transition from agent output, retrying up to [`MAX_TRANSITION_RETRIES`]
/// times automatically then waiting for user input if all attempts fail.
async fn parse_transition_with_retry<W: Write>(
    result_text: &str,
    session_id: Option<&str>,
    worktree_path: &Path,
    extra_args: &[String],
    system_prompt: Option<&str>,
    agents: &[AgentDef],
    ctx: &mut PhaseContext<'_, W>,
) -> Result<Option<Transition>> {
    let mut last_err = match transition::parse_transition(result_text) {
        Ok(t) => return Ok(Some(t)),
        Err(e) => e,
    };
    let Some(sid) = session_id else {
        return Err(last_err).context("failed to parse transition");
    };
    let mut current_sid = sid.to_string();

    for attempt in 1..=MAX_TRANSITION_RETRIES {
        let final_attempt = attempt == MAX_TRANSITION_RETRIES;

        ctx.renderer.write_raw(&format!(
            "\r\nTransition output could not be parsed: {last_err}\r\nRetrying ({attempt}/{MAX_TRANSITION_RETRIES})...\r\n\r\n"
        ));

        let retry_prompt = transition::corrective_prompt(&last_err, agents, final_attempt);

        let PhaseOutcome::Completed {
            result_text: retry_text,
            cost: retry_cost,
            session_id: retry_sid,
        } = run_phase_session(
            &retry_prompt,
            worktree_path,
            extra_args,
            Some(&current_sid),
            system_prompt,
            ctx,
        )
        .await?
        else {
            return Ok(None);
        };

        ctx.total_cost += retry_cost;
        if let Some(id) = retry_sid {
            current_sid = id;
        }

        match transition::parse_transition(&retry_text) {
            Ok(t) => return Ok(Some(t)),
            Err(e) => last_err = e,
        }
    }

    // All automatic attempts failed — wait for user input instead
    // of crashing. The user can talk to the agent to fix the situation.
    wait_for_transition_input(
        &last_err,
        current_sid,
        worktree_path,
        extra_args,
        system_prompt,
        ctx,
    )
    .await
}

/// When automatic transition parsing fails, wait for user input and retry.
///
/// Loops until the agent produces a valid `<next>` tag or the user exits.
async fn wait_for_transition_input<W: Write>(
    initial_err: &anyhow::Error,
    mut session_id: String,
    worktree_path: &Path,
    extra_args: &[String],
    system_prompt: Option<&str>,
    ctx: &mut PhaseContext<'_, W>,
) -> Result<Option<Transition>> {
    let mut last_err = format!("{initial_err}");

    loop {
        ctx.renderer.write_raw(&format!(
            "\r\nTransition output could not be parsed: {last_err}\r\n"
        ));

        let Some(text) = event_loop::wait_for_interrupt_input(
            ctx.input,
            ctx.renderer,
            ctx.io,
            ctx.vcr,
            &session_id,
            Some(worktree_path),
            extra_args,
        )
        .await?
        else {
            return Ok(None);
        };

        let PhaseOutcome::Completed {
            result_text,
            cost,
            session_id: new_sid,
        } = run_phase_session(
            &text,
            worktree_path,
            extra_args,
            Some(&session_id),
            system_prompt,
            ctx,
        )
        .await?
        else {
            return Ok(None);
        };

        ctx.total_cost += cost;
        if let Some(id) = new_sid {
            session_id = id;
        }

        match transition::parse_transition(&result_text) {
            Ok(t) => return Ok(Some(t)),
            Err(e) => {
                last_err = format!("{e}");
            }
        }
    }
}

/// Load `.coven/system.md` if it exists, empty string otherwise.
async fn vcr_load_system_doc(vcr: &VcrContext, wt_str: &str) -> Result<String> {
    vcr.call(
        "load_system_doc",
        wt_str.to_string(),
        async |p: &String| -> Result<String> {
            let path = Path::new(p).join(".coven/system.md");
            match std::fs::read_to_string(&path) {
                Ok(contents) => Ok(contents),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
                Err(e) => Err(e.into()),
            }
        },
    )
    .await
}

/// VCR-wrapped `worktree::main_branch_name`.
async fn vcr_main_branch_name(vcr: &VcrContext, wt_str: &str) -> Result<String> {
    vcr.call(
        "worktree::main_branch_name",
        wt_str.to_string(),
        async |p: &String| Ok(worktree::main_branch_name(Path::new(p))?),
    )
    .await
}

async fn vcr_main_head_sha(vcr: &VcrContext, wt_str: String) -> Result<String> {
    vcr.call("main_head_sha", wt_str, async |p: &String| {
        main_head_sha(Path::new(p))
    })
    .await
}

/// VCR-wrapped agent loading.
async fn vcr_load_agents(vcr: &VcrContext, worktree_path: &Path) -> Result<Vec<AgentDef>> {
    let agents_dir = worktree_path.join(agents::AGENTS_DIR);
    let agents_dir_str = agents_dir.display().to_string();
    let agent_defs = vcr
        .call("agents::load_agents", agents_dir_str, async |d: &String| {
            agents::load_agents(Path::new(d))
        })
        .await?;
    if agent_defs.is_empty() {
        bail!("no agent definitions found in {}", agents_dir.display());
    }
    Ok(agent_defs)
}

/// VCR-wrapped `semaphore::acquire`. Returns `None` if the agent has no
/// `max_concurrency` set (unlimited concurrency).
async fn vcr_acquire_semaphore(
    vcr: &VcrContext,
    wt_str: &str,
    agent_name: &str,
    agent_def: &AgentDef,
) -> Result<Option<semaphore::SemaphorePermit>> {
    let Some(max) = agent_def.frontmatter.max_concurrency else {
        return Ok(None);
    };
    let permit = vcr
        .call(
            &format!("semaphore::acquire::{agent_name}"),
            SemaphoreAcquireArgs {
                path: wt_str.to_string(),
                agent: agent_name.to_string(),
                max_concurrency: max,
            },
            async |a: &SemaphoreAcquireArgs| {
                semaphore::acquire(Path::new(&a.path), &a.agent, a.max_concurrency).await
            },
        )
        .await?;
    Ok(Some(permit))
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

/// Run an interactive claude session for a worker phase.
///
/// If `resume` is provided, the session is resumed from the given session ID
/// rather than starting fresh. `system_prompt` is set via `--append-system-prompt`.
async fn run_phase_session<W: Write>(
    prompt: &str,
    working_dir: &Path,
    extra_args: &[String],
    resume: Option<&str>,
    system_prompt: Option<&str>,
    ctx: &mut PhaseContext<'_, W>,
) -> Result<PhaseOutcome> {
    let append_system_prompt = system_prompt.map(String::from).or_else(|| {
        ctx.fork_config
            .map(|_| fork::fork_system_prompt().to_string())
    });
    let session_config = SessionConfig {
        prompt: Some(prompt.to_string()),
        extra_args: extra_args.to_vec(),
        append_system_prompt,
        resume: resume.map(String::from),
        working_dir: Some(working_dir.to_path_buf()),
    };

    let mut runner = event_loop::spawn_session(session_config.clone(), ctx.io, ctx.vcr).await?;
    let mut state = SessionState::default();

    loop {
        let outcome = event_loop::run_session(
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
                let Some(session_id) = state.session_id.take() else {
                    return Ok(PhaseOutcome::Exited);
                };
                ctx.renderer.render_interrupted();

                let Some(text) = event_loop::wait_for_interrupt_input(
                    ctx.input,
                    ctx.renderer,
                    ctx.io,
                    ctx.vcr,
                    &session_id,
                    Some(working_dir),
                    extra_args,
                )
                .await?
                else {
                    return Ok(PhaseOutcome::Exited);
                };
                let resume_config = session_config.resume_with(text, session_id.clone());
                runner = event_loop::spawn_session(resume_config, ctx.io, ctx.vcr).await?;
                state = SessionState::default();
                state.session_id = Some(session_id);
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
    ref_paths: Option<RefPaths>,
) -> Result<(notify::RecommendedWatcher, tokio::sync::mpsc::Receiver<()>)> {
    let (tx, rx) = tokio::sync::mpsc::channel(1);

    let mut watcher = notify::recommended_watcher(move |_: notify::Result<notify::Event>| {
        // Best-effort send; if the channel is full, a notification is already pending.
        let _ = tx.try_send(());
    })
    .context("failed to create filesystem watcher")?;

    if let Some(paths) = ref_paths {
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

#[derive(Serialize, Deserialize)]
struct RefPaths {
    refs_heads_dir: PathBuf,
    loose_ref: PathBuf,
    packed_refs: PathBuf,
}

/// Resolve the git ref paths to watch. Returns `None` if the git commands fail.
fn resolve_ref_paths(worktree_path: &Path) -> Option<RefPaths> {
    let main_branch = worktree::main_branch_name(worktree_path).ok()?;
    let git_common_dir = worktree::git_common_dir(worktree_path).ok()?;

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

    // Set up watcher BEFORE reading baseline HEAD to avoid TOCTOU race:
    // a commit between baseline capture and watcher setup would be missed.
    let ref_paths = vcr
        .call("resolve_ref_paths", wt_str.clone(), async |p: &String| {
            Ok(resolve_ref_paths(Path::new(p)))
        })
        .await?;
    // _watcher must stay alive for the duration of the loop.
    let (_watcher, mut rx) = setup_ref_watcher(ref_paths)?;

    // Read baseline after watcher setup: any commit after the watcher is
    // active will fire a notification, and any commit before this read is
    // already reflected in baseline_head.
    let baseline_head = vcr_main_head_sha(vcr, wt_str.clone()).await?;

    vcr.call("idle", (), async |(): &()| Ok(())).await?;

    loop {
        tokio::select! {
            _ = rx.recv() => {
                let current = vcr_main_head_sha(vcr, wt_str.clone()).await?;
                if current != baseline_head {
                    renderer.write_raw("New commits detected on main.\r\n");
                    return Ok(WaitOutcome::NewCommits);
                }
                // Spurious notification — loop and wait again
            }
            event = vcr.call("next_event", (), async |(): &()| io.next_event().await) => {
                let event = event?;
                if let IoEvent::Terminal(Event::Key(key_event)) = event {
                    let action = input.handle_key(&key_event, renderer.writer());
                    match action {
                        InputAction::Interrupt | InputAction::EndSession => {
                            return Ok(WaitOutcome::Exited);
                        }
                        InputAction::ViewMessage(ref query) => {
                            event_loop::view_message(renderer, query, io)?;
                        }
                        _ => {}
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
