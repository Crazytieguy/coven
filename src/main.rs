mod cli;
mod commands;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};

#[tokio::main]
async fn main() -> Result<()> {
    // Panic hook to restore terminal state
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        crossterm::terminal::disable_raw_mode().ok();
        default_hook(info);
    }));

    let cli = Cli::parse();

    match cli.command {
        Some(Command::Init) => {
            commands::init::init()?;
        }
        Some(Command::Status) => {
            commands::status::status()?;
        }
        Some(Command::Ralph {
            prompt,
            iterations,
            break_tag,
            no_break,
            show_thinking,
            claude_args,
        }) => {
            if no_break && iterations == 0 {
                anyhow::bail!("--no-break requires --iterations to prevent infinite looping");
            }
            commands::ralph::ralph(commands::ralph::RalphConfig {
                prompt,
                iterations,
                break_tag,
                no_break,
                show_thinking,
                extra_args: claude_args,
            })
            .await?;
        }
        Some(Command::Worker {
            branch,
            worktree_base,
            show_thinking,
            claude_args,
        }) => {
            let base = match worktree_base {
                Some(b) => b,
                None => default_worktree_base()?,
            };
            commands::worker::worker(commands::worker::WorkerConfig {
                show_thinking,
                branch,
                worktree_base: base,
                extra_args: claude_args,
            })
            .await?;
        }
        None => {
            commands::run::run(cli.prompt, cli.claude_args, cli.show_thinking).await?;
        }
    }

    Ok(())
}

fn default_worktree_base() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| {
        anyhow::anyhow!("HOME not set; use --worktree-base to specify worktree location")
    })?;
    Ok(PathBuf::from(home).join("worktrees"))
}
