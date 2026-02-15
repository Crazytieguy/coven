use std::io::Write;
use std::path::Path;
use std::process::Command as StdCommand;

use anyhow::{Context, Result, bail};
use crossterm::event::{Event, KeyEvent};
use crossterm::terminal;

use crate::display::input::{InputAction, InputHandler};
use crate::display::renderer::Renderer;
use crate::event::{AppEvent, InputMode};
use crate::fork::{self, ForkConfig};
use crate::protocol::types::{AssistantContentBlock, InboundEvent, SystemEvent};
use crate::session::runner::{SessionConfig, SessionRunner};
use crate::session::state::{SessionState, SessionStatus};
use crate::vcr::{Io, IoEvent, VcrContext};

/// Send a message to the session via VCR.
async fn vcr_send_message(
    runner: &mut SessionRunner,
    vcr: &VcrContext,
    message: String,
) -> Result<()> {
    vcr.call("send_message", message, async |t: &String| {
        runner.send_message(t).await
    })
    .await
}

/// How a session ended.
pub enum SessionOutcome {
    /// Session produced a result (normal completion).
    Completed { result_text: String },
    /// User pressed Ctrl+C.
    Interrupted,
    /// Claude process exited unexpectedly.
    ProcessExited,
}

/// Per-session transient state for event buffering and follow-ups.
struct SessionLocals {
    event_buffer: Vec<AppEvent>,
    pending_followups: Vec<String>,
    result_text: String,
    fork_config: Option<ForkConfig>,
}

/// Run a single session's event loop with full input support.
///
/// Handles streaming display, event buffering during input, steering messages,
/// follow-up messages, message viewing, and interrupt/end-session signals.
///
/// Returns when the session produces a Result event, the user interrupts,
/// or the process exits.
pub async fn run_session<W: Write>(
    runner: &mut SessionRunner,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
    input: &mut InputHandler,
    io: &mut Io,
    vcr: &VcrContext,
    fork_config: Option<&ForkConfig>,
) -> Result<SessionOutcome> {
    let mut locals = SessionLocals {
        event_buffer: Vec::new(),
        pending_followups: Vec::new(),
        result_text: String::new(),
        fork_config: fork_config.cloned(),
    };

    loop {
        let io_event: IoEvent = vcr
            .call("next_event", (), async |(): &()| io.next_event().await)
            .await?;
        match io_event {
            IoEvent::Claude(app_event) => {
                if input.is_active() && state.status == SessionStatus::Running {
                    locals.event_buffer.push(app_event);
                } else {
                    let outcome = process_claude_event(
                        app_event,
                        state,
                        renderer,
                        runner,
                        &mut locals,
                        vcr,
                        fork_config,
                    )
                    .await?;
                    if let Some(outcome) = outcome {
                        return Ok(outcome);
                    }
                }
            }
            IoEvent::Terminal(Event::Key(key_event)) => {
                let action = handle_session_key_event(
                    &key_event,
                    input,
                    renderer,
                    runner,
                    state,
                    &mut locals,
                    vcr,
                )
                .await?;
                match action {
                    LoopAction::Continue => {}
                    LoopAction::Return(outcome) => return Ok(outcome),
                    LoopAction::ViewMessage(ref query) => {
                        view_message(renderer, query, io)?;
                        let flush = flush_event_buffer(&mut locals, state, renderer);
                        if let FlushResult::Completed(ref result_text) = flush {
                            return Ok(SessionOutcome::Completed {
                                result_text: result_text.clone(),
                            });
                        }
                        let fork_cfg = locals.fork_config.as_ref();
                        if let Some(LoopAction::Return(outcome)) =
                            handle_flush_result(flush, state, renderer, runner, vcr, fork_cfg)
                                .await?
                        {
                            return Ok(outcome);
                        }
                        // Don't re-activate input — it was deactivated when the
                        // user submitted the :N command. The user returns to
                        // inactive state and can type a character to start new
                        // input naturally (via Activated(c)).
                    }
                }
            }
            IoEvent::Terminal(_) => {}
        }
    }
}

/// Flow control signals from key event handlers.
enum LoopAction {
    Continue,
    Return(SessionOutcome),
    /// Open a message in the pager — deferred to `run_session` which has `io`.
    ViewMessage(String),
}

/// What happened during event buffer flush that requires caller action.
enum FlushResult {
    /// No special action needed.
    Continue,
    /// A pending followup was dequeued after a buffered Result.
    /// Caller should send it and set state to Running.
    Followup(String),
    /// A Result was flushed with no pending followups — session completed.
    Completed(String),
    /// A fork tag was detected in a buffered Result event.
    Fork(Vec<String>),
    /// The process exited during the flush.
    ProcessExited,
}

/// What action to take after classifying a Claude inbound event.
enum ClaudeEventAction {
    /// Normal event (not a Result), already rendered. No further action.
    Rendered,
    /// Result with fork tasks detected.
    Fork(Vec<String>),
    /// Result with a pending followup to send.
    Followup(String),
    /// Result with no followups — session completed.
    Completed(String),
}

/// Handle an inbound Claude event, updating session state and rendering output.
///
/// When `has_pending_followups` is true, Result events update state but skip
/// rendering the Done line — the follow-up will continue the conversation.
fn handle_inbound<W: Write>(
    event: &InboundEvent,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
    has_pending_followups: bool,
) {
    match event {
        InboundEvent::System(SystemEvent::Init(init)) => {
            let same_session = state.session_id.as_deref() == Some(&init.session_id);
            state.session_id = Some(init.session_id.clone());
            state.status = SessionStatus::Running;
            if same_session {
                if state.suppress_next_separator {
                    state.suppress_next_separator = false;
                } else {
                    renderer.render_turn_separator();
                }
            } else {
                state.suppress_next_separator = false;
                renderer.render_session_header(&init.session_id, &init.model);
            }
        }
        InboundEvent::System(SystemEvent::Status { status: Some(s) }) if s == "compacting" => {
            renderer.render_compaction();
        }
        InboundEvent::System(SystemEvent::Status { .. } | SystemEvent::Other) => {}
        InboundEvent::StreamEvent(se) => {
            renderer.handle_stream_event(se);
        }
        InboundEvent::Assistant(msg) => {
            if let Some(ref parent_id) = msg.parent_tool_use_id {
                for block in &msg.message.content {
                    if let AssistantContentBlock::ToolUse { name, input, .. } = block {
                        renderer.render_subagent_tool_call(name, input, parent_id);
                    }
                }
            }
        }
        InboundEvent::User(u) => {
            if u.parent_tool_use_id.is_some() {
                if let Some(ref message) = u.message {
                    renderer.render_subagent_tool_result(message);
                }
            } else if let Some(ref result) = u.tool_use_result {
                renderer.render_tool_result(result, u.message.as_ref());
            } else if renderer.is_compacting() {
                renderer.set_compaction_content(u.message.as_ref());
            }
        }
        InboundEvent::Result(result) => {
            state.total_cost_usd = result.total_cost_usd;
            state.status = SessionStatus::WaitingForInput;
            if !has_pending_followups {
                renderer.render_result(
                    &result.subtype,
                    result.total_cost_usd,
                    result.duration_ms,
                    result.num_turns,
                );
            }
        }
    }
}

/// Classify a Claude inbound event: capture result text, detect forks, render,
/// and determine what action the caller should take.
fn classify_claude_event<W: Write>(
    inbound: &InboundEvent,
    locals: &mut SessionLocals,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
    fork_config: Option<&ForkConfig>,
) -> ClaudeEventAction {
    if let InboundEvent::Result(ref result) = *inbound {
        locals.result_text.clone_from(&result.result);
    }

    let fork_tasks = if let InboundEvent::Result(_) = *inbound {
        fork_config.and_then(|_| fork::parse_fork_tag(&locals.result_text))
    } else {
        None
    };

    let has_pending = !locals.pending_followups.is_empty() || fork_tasks.is_some();
    handle_inbound(inbound, state, renderer, has_pending);

    if let Some(tasks) = fork_tasks {
        ClaudeEventAction::Fork(tasks)
    } else if matches!(*inbound, InboundEvent::Result(_)) {
        if locals.pending_followups.is_empty() {
            ClaudeEventAction::Completed(locals.result_text.clone())
        } else {
            let text = locals.pending_followups.remove(0);
            ClaudeEventAction::Followup(text)
        }
    } else {
        ClaudeEventAction::Rendered
    }
}

/// Handle a key event during an active session.
async fn handle_session_key_event<W: Write>(
    key_event: &KeyEvent,
    input: &mut InputHandler,
    renderer: &mut Renderer<W>,
    runner: &mut SessionRunner,
    state: &mut SessionState,
    locals: &mut SessionLocals,
    vcr: &VcrContext,
) -> Result<LoopAction> {
    let action = input.handle_key(key_event, renderer.writer());
    match action {
        InputAction::Activated(_) => {
            renderer.begin_input_line();
            input.redraw(renderer.writer());
        }
        InputAction::Submit(text, mode) => {
            let flush = flush_event_buffer(locals, state, renderer);
            // Completed is intentionally not special-cased here: if the session
            // completed during the flush, state is WaitingForInput and the match
            // below will send the user's text as a follow-up.
            let fork_cfg = locals.fork_config.as_ref();
            if let Some(action) =
                handle_flush_result(flush, state, renderer, runner, vcr, fork_cfg).await?
            {
                return Ok(action);
            }
            match mode {
                InputMode::Steering => {
                    renderer.render_steering_sent(&text);
                    vcr_send_message(runner, vcr, text).await?;
                }
                InputMode::FollowUp => {
                    if state.status == SessionStatus::WaitingForInput {
                        renderer.render_user_message(&text);
                        state.suppress_next_separator = true;
                        vcr_send_message(runner, vcr, text).await?;
                        state.status = SessionStatus::Running;
                    } else {
                        renderer.render_followup_queued(&text);
                        locals.pending_followups.push(text);
                    }
                }
            }
        }
        InputAction::ViewMessage(query) => {
            return Ok(LoopAction::ViewMessage(query));
        }
        InputAction::Interrupt => {
            runner.kill().await?;
            return Ok(LoopAction::Return(SessionOutcome::Interrupted));
        }
        InputAction::EndSession => {
            runner.close_input();
        }
        InputAction::Cancel => {
            let flush = flush_event_buffer(locals, state, renderer);
            if let FlushResult::Completed(ref result_text) = flush {
                return Ok(LoopAction::Return(SessionOutcome::Completed {
                    result_text: result_text.clone(),
                }));
            }
            let fork_cfg = locals.fork_config.as_ref();
            if let Some(action) =
                handle_flush_result(flush, state, renderer, runner, vcr, fork_cfg).await?
            {
                return Ok(action);
            }
            if state.status == SessionStatus::WaitingForInput {
                renderer.show_prompt();
                input.activate();
            }
        }
        InputAction::Interactive | InputAction::None => {}
    }
    Ok(LoopAction::Continue)
}

/// Send a follow-up message: render indicator, suppress separator, send, set Running.
async fn send_followup<W: Write>(
    text: String,
    renderer: &mut Renderer<W>,
    runner: &mut SessionRunner,
    state: &mut SessionState,
    vcr: &VcrContext,
) -> Result<()> {
    renderer.render_followup_sent(&text);
    state.suppress_next_separator = true;
    vcr_send_message(runner, vcr, text).await?;
    state.status = SessionStatus::Running;
    Ok(())
}

/// Execute a fork: run children, send reintegration result, set Running.
async fn execute_fork<W: Write>(
    tasks: Vec<String>,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
    runner: &mut SessionRunner,
    vcr: &VcrContext,
    fork_config: Option<&ForkConfig>,
) -> Result<()> {
    let session_id = state
        .session_id
        .clone()
        .context("cannot fork: no session ID yet")?;
    let Some(fork_cfg) = fork_config else {
        bail!("fork detected but fork_config is None");
    };
    let msg = fork::run_fork(&session_id, tasks, fork_cfg, renderer, vcr).await?;
    vcr_send_message(runner, vcr, msg).await?;
    state.suppress_next_separator = true;
    state.status = SessionStatus::Running;
    Ok(())
}

/// Process a single claude event. Returns Some(outcome) if the session should end.
async fn process_claude_event<W: Write>(
    event: AppEvent,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
    runner: &mut SessionRunner,
    locals: &mut SessionLocals,
    vcr: &VcrContext,
    fork_config: Option<&ForkConfig>,
) -> Result<Option<SessionOutcome>> {
    match event {
        AppEvent::Claude(inbound) => {
            match classify_claude_event(&inbound, locals, state, renderer, fork_config) {
                ClaudeEventAction::Fork(tasks) => {
                    execute_fork(tasks, state, renderer, runner, vcr, fork_config).await?;
                }
                ClaudeEventAction::Followup(text) => {
                    send_followup(text, renderer, runner, state, vcr).await?;
                }
                ClaudeEventAction::Completed(result_text) => {
                    return Ok(Some(SessionOutcome::Completed { result_text }));
                }
                ClaudeEventAction::Rendered => {}
            }
        }
        AppEvent::ParseWarning(warning) | AppEvent::Stderr(warning) => {
            renderer.render_warning(&warning);
        }
        AppEvent::ProcessExit(code) => {
            renderer.render_exit(code);
            state.status = SessionStatus::Ended;
            return Ok(Some(SessionOutcome::ProcessExited));
        }
    }
    Ok(None)
}

/// Flush all buffered events through the renderer.
///
/// Returns a `FlushResult` indicating whether the caller needs to take action:
/// sending a dequeued followup, handling a completion, or handling a process exit.
/// `ProcessExit` only takes effect if no higher-priority result (e.g. `Completed`,
/// `Followup`) was already determined from earlier events in the buffer.
fn flush_event_buffer<W: Write>(
    locals: &mut SessionLocals,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
) -> FlushResult {
    let mut result = FlushResult::Continue;
    let buffered: Vec<_> = locals.event_buffer.drain(..).collect();
    // Clone once to avoid overlapping borrows (fork_config is small)
    let fork_config = locals.fork_config.clone();
    for event in buffered {
        match event {
            AppEvent::Claude(inbound) => {
                match classify_claude_event(&inbound, locals, state, renderer, fork_config.as_ref())
                {
                    ClaudeEventAction::Fork(tasks) => result = FlushResult::Fork(tasks),
                    ClaudeEventAction::Followup(text) => result = FlushResult::Followup(text),
                    ClaudeEventAction::Completed(text) => result = FlushResult::Completed(text),
                    ClaudeEventAction::Rendered => {}
                }
            }
            AppEvent::ParseWarning(warning) | AppEvent::Stderr(warning) => {
                renderer.render_warning(&warning);
            }
            AppEvent::ProcessExit(code) => {
                renderer.render_exit(code);
                state.status = SessionStatus::Ended;
                if matches!(result, FlushResult::Continue) {
                    result = FlushResult::ProcessExited;
                }
            }
        }
    }
    result
}

/// Handle the result of flushing the event buffer: send a dequeued followup
/// or return early on process exit.
async fn handle_flush_result<W: Write>(
    flush: FlushResult,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
    runner: &mut SessionRunner,
    vcr: &VcrContext,
    fork_config: Option<&ForkConfig>,
) -> Result<Option<LoopAction>> {
    match flush {
        FlushResult::ProcessExited => Ok(Some(LoopAction::Return(SessionOutcome::ProcessExited))),
        FlushResult::Followup(text) => {
            send_followup(text, renderer, runner, state, vcr).await?;
            Ok(None)
        }
        FlushResult::Fork(tasks) => {
            execute_fork(tasks, state, renderer, runner, vcr, fork_config).await?;
            Ok(None)
        }
        FlushResult::Completed(_) | FlushResult::Continue => Ok(None),
    }
}

/// What the user chose to do after a session completed.
pub enum FollowUpAction {
    /// User sent a follow-up message; continue the session.
    Sent,
    /// User wants to end the session (Ctrl+D, Ctrl+C, etc.).
    Exit,
}

/// Result of waiting for user input in the interrupted state.
pub enum WaitResult {
    /// User submitted text to resume the session with.
    Text(String),
    /// User wants to drop into the native Claude TUI.
    Interactive,
}

/// Show a prompt and wait for user to type a follow-up or exit.
pub async fn wait_for_followup<W: Write>(
    input: &mut InputHandler,
    renderer: &mut Renderer<W>,
    runner: &mut SessionRunner,
    state: &mut SessionState,
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<FollowUpAction> {
    renderer.write_raw("\x07");
    vcr.call("idle", (), async |(): &()| Ok(())).await?;
    loop {
        match wait_for_text_input(input, renderer, io, vcr).await? {
            Some(WaitResult::Text(text)) => {
                state.suppress_next_separator = true;
                vcr_send_message(runner, vcr, text).await?;
                state.status = SessionStatus::Running;
                return Ok(FollowUpAction::Sent);
            }
            Some(WaitResult::Interactive) => {
                // Interactive not applicable in follow-up state; ignore and re-prompt.
            }
            None => return Ok(FollowUpAction::Exit),
        }
    }
}

/// Show a prompt and wait for user input. Returns the text, Interactive, or None to exit.
///
/// Unlike `wait_for_followup`, this doesn't send the message to a runner —
/// the caller decides what to do with the text (e.g. spawn a resumed session).
pub async fn wait_for_user_input<W: Write>(
    input: &mut InputHandler,
    renderer: &mut Renderer<W>,
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<Option<WaitResult>> {
    wait_for_text_input(input, renderer, io, vcr).await
}

/// Wait for user input from the interrupted state, handling Ctrl+O to open
/// an interactive session. Returns the resume text, or None to exit.
pub async fn wait_for_interrupt_input<W: Write>(
    input: &mut InputHandler,
    renderer: &mut Renderer<W>,
    io: &mut Io,
    vcr: &VcrContext,
    session_id: &str,
    working_dir: Option<&Path>,
    extra_args: &[String],
) -> Result<Option<String>> {
    io.clear_event_channel();
    vcr.call("idle", (), async |(): &()| Ok(())).await?;
    loop {
        match wait_for_text_input(input, renderer, io, vcr).await? {
            Some(WaitResult::Text(text)) => return Ok(Some(text)),
            Some(WaitResult::Interactive) => {
                open_interactive_session(session_id, working_dir, extra_args, io, vcr)?;
                renderer.render_returned_from_interactive();
            }
            None => return Ok(None),
        }
    }
}

/// Wait for user to type and submit text, request interactive mode, or exit.
///
/// Shows the prompt, activates input, and loops on events.
/// Returns the submitted text / interactive request, or None if the user interrupted/ended.
async fn wait_for_text_input<W: Write>(
    input: &mut InputHandler,
    renderer: &mut Renderer<W>,
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<Option<WaitResult>> {
    renderer.show_prompt();
    input.activate();

    loop {
        let io_event: IoEvent = vcr
            .call("next_event", (), async |(): &()| io.next_event().await)
            .await?;
        match io_event {
            IoEvent::Terminal(Event::Key(key_event)) => {
                let action = input.handle_key(&key_event, renderer.writer());
                match action {
                    InputAction::Submit(text, _) => {
                        renderer.render_user_message(&text);
                        return Ok(Some(WaitResult::Text(text)));
                    }
                    InputAction::Interactive => {
                        return Ok(Some(WaitResult::Interactive));
                    }
                    InputAction::ViewMessage(ref query) => {
                        view_message(renderer, query, io)?;
                    }
                    InputAction::Cancel => {
                        renderer.show_prompt();
                        input.activate();
                    }
                    InputAction::Interrupt | InputAction::EndSession => {
                        return Ok(None);
                    }
                    InputAction::Activated(_) => {
                        renderer.begin_input_line();
                        input.redraw(renderer.writer());
                    }
                    InputAction::None => {}
                }
            }
            IoEvent::Claude(AppEvent::ProcessExit(_)) => return Ok(None),
            IoEvent::Terminal(_) | IoEvent::Claude(_) => {}
        }
    }
}

/// Spawn a new Claude session via VCR.
pub async fn spawn_session(
    config: SessionConfig,
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<SessionRunner> {
    vcr.call("spawn", config, async |c: &SessionConfig| {
        let tx = io.replace_event_channel();
        SessionRunner::spawn(c.clone(), tx).await
    })
    .await
}

/// Restore terminal state after exclusive access (interactive session or pager).
///
/// Flushes buffered input from the kernel queue, drains any residual events
/// queued before the pause took effect, resumes the background terminal reader,
/// and re-enables raw mode.
fn restore_terminal(io: &mut Io) -> Result<()> {
    // SAFETY: tcflush on STDIN_FILENO with TCIFLUSH is a POSIX syscall that
    // discards buffered input bytes — no memory or resource safety concerns.
    unsafe { libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH) };
    io.drain_term_events();
    io.resume_term_reader();
    terminal::enable_raw_mode().context("failed to re-enable raw mode")?;
    Ok(())
}

/// Drop into the native Claude Code TUI to continue a session interactively.
///
/// Temporarily exits raw mode, spawns `claude --resume <session_id>` as a
/// blocking child process, waits for it to exit, and re-enables raw mode.
/// Pauses the background terminal reader so the child gets exclusive stdin access.
pub fn open_interactive_session(
    session_id: &str,
    working_dir: Option<&Path>,
    extra_args: &[String],
    io: &mut Io,
    vcr: &VcrContext,
) -> Result<()> {
    if !vcr.is_live() {
        bail!("interactive sessions are not supported in VCR replay mode");
    }

    // Pause the background terminal reader so the child process gets
    // exclusive access to stdin — prevents keypress competition.
    io.pause_term_reader();

    terminal::disable_raw_mode().context("failed to disable raw mode for interactive session")?;
    print!("\r\n[opening interactive session — exit to return]\r\n");

    let filtered_args: Vec<&String> = extra_args
        .iter()
        .filter(|a| *a != "-p" && *a != "--output-format" && !a.starts_with("--output-format="))
        .collect();

    let mut cmd = StdCommand::new("claude");
    cmd.args(["--resume", session_id]);
    cmd.args(filtered_args);
    cmd.env_remove("CLAUDECODE");
    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }

    let status = cmd
        .status()
        .context("failed to spawn claude for interactive session")?;
    if !status.success()
        && let Some(code) = status.code()
    {
        eprintln!("claude exited with code {code}");
    }

    restore_terminal(io)?;

    Ok(())
}

/// Open a message in $PAGER, looked up by label query (e.g. "3" or "2/1").
///
/// Pauses the background terminal reader so the pager gets exclusive stdin
/// access — same pattern as [`open_interactive_session`].
pub fn view_message<W: Write>(renderer: &mut Renderer<W>, query: &str, io: &mut Io) -> Result<()> {
    use crate::display::renderer::format_message;

    let Some(mut content) = format_message(renderer.messages(), query) else {
        renderer.write_raw(&format!("No message {query}\r\n"));
        return Ok(());
    };

    // Pad short content with trailing newlines so the pager shows it top-aligned.
    if let Ok((_, rows)) = terminal::size() {
        let line_count = content.chars().filter(|&c| c == '\n').count() + 1;
        if line_count < rows as usize {
            content.extend(std::iter::repeat_n('\n', rows as usize - line_count));
        }
    }

    // Pause the background terminal reader so the pager gets exclusive
    // access to stdin — prevents keypress competition.
    io.pause_term_reader();

    // Leave raw mode so the pager can handle keyboard input.
    // The pager manages its own alternate screen.
    terminal::disable_raw_mode().ok();

    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less".to_string());
    let mut child = match StdCommand::new(&pager)
        .arg("-R") // handle ANSI colors
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            restore_terminal(io)?;
            renderer.write_raw(&format!("Failed to open pager '{pager}': {e}\r\n"));
            return Ok(());
        }
    };

    if let Some(ref mut stdin) = child.stdin
        && let Err(e) = stdin.write_all(content.as_bytes())
    {
        // Not fatal — pager may have quit early (broken pipe). Log and continue
        // so we still wait on the child and restore terminal state.
        eprintln!("pager write error: {e}");
    }
    // Close stdin so pager reads EOF
    child.stdin.take();
    child.wait().ok();

    restore_terminal(io)?;
    Ok(())
}
