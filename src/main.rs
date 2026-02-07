mod cli;
mod commands;

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
        Some(Command::Ralph {
            prompt,
            iterations,
            break_tag,
            no_break,
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
                extra_args: claude_args,
            })
            .await?;
        }
        None => {
            commands::run::run(cli.prompt, cli.claude_args).await?;
        }
    }

    Ok(())
}
