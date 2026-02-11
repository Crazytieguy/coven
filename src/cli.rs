use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Claude session options shared across all command modes.
#[derive(clap::Args, Debug)]
pub struct ClaudeOpts {
    /// Stream thinking text inline in dim italic instead of collapsing.
    #[arg(long)]
    pub show_thinking: bool,

    /// Enable model-driven context forking via <fork> tags.
    #[arg(long)]
    pub fork: bool,

    /// Extra arguments to pass through to claude (after --).
    #[arg(last = true)]
    pub claude_args: Vec<String>,
}

#[derive(Parser, Debug)]
#[command(
    name = "coven",
    about = "A minimal streaming display and workflow runner for Claude Code's -p mode",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Prompt to send to claude.
    #[arg(value_name = "PROMPT")]
    pub prompt: Option<String>,

    #[command(flatten)]
    pub claude_opts: ClaudeOpts,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run claude in a loop with filesystem state accumulation.
    Ralph {
        /// Prompt to send to claude on each iteration.
        #[arg(value_name = "PROMPT")]
        prompt: String,

        /// Maximum number of iterations (0 = infinite).
        #[arg(long, default_value = "0")]
        iterations: u32,

        /// Tag that signals loop completion.
        #[arg(long, default_value = "break")]
        break_tag: String,

        /// Disable break tag detection (requires --iterations to prevent infinite loop).
        #[arg(long)]
        no_break: bool,

        #[command(flatten)]
        claude_opts: ClaudeOpts,
    },

    /// Initialize project with default agent prompts and directory structure.
    Init,

    /// Show status of all active workers.
    Status,

    /// Remove orphaned worktrees left behind by dead workers.
    Gc,

    /// Start an orchestration worker (dispatch → agent → land loop).
    Worker {
        /// Branch name for the worktree (random if not specified).
        #[arg(long)]
        branch: Option<String>,

        /// Base directory for worktrees. Default: ~/worktrees.
        #[arg(long)]
        worktree_base: Option<PathBuf>,

        #[command(flatten)]
        claude_opts: ClaudeOpts,
    },
}
