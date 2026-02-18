mod cli;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use coven::commands;
use coven::vcr::{Io, VcrContext};

use cli::{Cli, Command};

#[tokio::main]
async fn main() -> Result<()> {
    install_panic_hook();
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Init) => {
            let vcr = VcrContext::live();
            commands::init::init(
                &vcr,
                &mut std::io::stdout(),
                &mut std::io::stdin().lock(),
                None,
            )
            .await?;
        }
        Some(Command::Status) => {
            let vcr = VcrContext::live();
            commands::status::status(&vcr, None, &mut std::io::stdout()).await?;
        }
        Some(Command::Gc { force }) => {
            let vcr = VcrContext::live();
            commands::gc::gc(&vcr, force, None, &mut std::io::stdout()).await?;
        }
        Some(Command::Ralph {
            prompt,
            iterations,
            break_tag,
            no_break,
            claude_opts,
        }) => {
            if no_break && iterations == 0 {
                anyhow::bail!("--no-break requires --iterations to prevent infinite looping");
            }
            let (mut io, vcr) = create_live_io();
            commands::ralph::ralph(
                commands::ralph::RalphConfig {
                    prompt,
                    iterations,
                    break_tag,
                    no_break,
                    show_thinking: claude_opts.show_thinking,
                    tag_flags: commands::ralph::TagFlags {
                        fork: claude_opts.fork,
                        reload: claude_opts.reload,
                    },
                    extra_args: claude_opts.claude_args,
                    working_dir: None,
                    term_width: None,
                },
                &mut io,
                &vcr,
                std::io::stdout(),
            )
            .await?;
        }
        Some(Command::Worker {
            branch,
            worktree_base,
            claude_opts,
        }) => {
            let base = match worktree_base {
                Some(b) => b,
                None => default_worktree_base()?,
            };
            let (mut io, vcr) = create_live_io();
            commands::worker::worker(
                commands::worker::WorkerConfig {
                    show_thinking: claude_opts.show_thinking,
                    branch,
                    worktree_base: base,
                    extra_args: claude_opts.claude_args,
                    working_dir: None,
                    fork: claude_opts.fork,
                    reload: claude_opts.reload,
                    term_width: None,
                },
                &mut io,
                &vcr,
                std::io::stdout(),
            )
            .await?;
        }
        None => {
            let (mut io, vcr) = create_live_io();
            commands::run::run(
                commands::run::RunConfig {
                    prompt: cli.prompt,
                    extra_args: cli.claude_opts.claude_args,
                    show_thinking: cli.claude_opts.show_thinking,
                    fork: cli.claude_opts.fork,
                    reload: cli.claude_opts.reload,
                    working_dir: None,
                    term_width: None,
                },
                &mut io,
                &vcr,
                std::io::stdout(),
            )
            .await?;
        }
    }

    Ok(())
}

/// Install a panic hook that restores terminal state before printing the panic.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        crossterm::terminal::disable_raw_mode().ok();
        default_hook(info);
    }));
}

/// Create a live `Io` and `VcrContext` for production use.
///
/// Spawns a background task that reads crossterm events and forwards them
/// to the terminal event channel. The event channel starts empty — the first
/// `SessionRunner::spawn` should provide claude events via `io.replace_event_channel()`.
fn create_live_io() -> (Io, VcrContext) {
    use crossterm::event::EventStream;
    use futures::StreamExt;
    use tokio::sync::{mpsc, watch};

    let (term_tx, term_rx) = mpsc::unbounded_channel();
    let (_event_tx, event_rx) = mpsc::unbounded_channel();
    let (gate_tx, mut gate_rx) = watch::channel(true);

    // Background task: forward crossterm events to the channel.
    // Respects the pause gate — when paused, drops the EventStream to
    // release stdin (and its internal parser state), then waits for the
    // gate to become true and recreates the stream fresh.
    //
    // Uses `tokio::select!` so the gate change is noticed immediately,
    // even while blocked in `stream.next()`. Without this, the reader
    // would hold the EventStream (and compete for stdin) until the next
    // terminal event arrives — causing stale parser state that can
    // swallow the first character typed after returning from a child
    // process (e.g. the native Claude TUI via Ctrl+O).
    tokio::spawn(async move {
        let mut stream = EventStream::new();
        loop {
            tokio::select! {
                result = stream.next() => {
                    match result {
                        Some(Ok(event)) => {
                            if term_tx.send(event).is_err() {
                                return;
                            }
                        }
                        Some(Err(_)) | None => return,
                    }
                }
                result = gate_rx.changed() => {
                    if result.is_err() {
                        return; // gate sender dropped, shut down
                    }
                    if !*gate_rx.borrow() {
                        // Paused: drop the stream to release stdin
                        drop(stream);
                        // Wait for resume signal
                        loop {
                            if gate_rx.changed().await.is_err() {
                                return;
                            }
                            if *gate_rx.borrow() {
                                break;
                            }
                        }
                        // Recreate the stream after resuming
                        stream = EventStream::new();
                    }
                }
            }
        }
    });

    let mut io = Io::new(event_rx, term_rx);
    io.set_term_gate(gate_tx);
    // Keep the event channel alive so recv() blocks instead of
    // returning ProcessExit immediately (the sender was dropped above).
    io.clear_event_channel();
    let vcr = VcrContext::live();
    (io, vcr)
}

fn default_worktree_base() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| {
        anyhow::anyhow!("HOME not set; use --worktree-base to specify worktree location")
    })?;
    Ok(PathBuf::from(home).join(".coven").join("worktrees"))
}
