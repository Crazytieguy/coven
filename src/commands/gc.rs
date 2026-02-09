use std::collections::HashSet;

use anyhow::{Context, Result};

use coven::worker_state;
use coven::worktree;

/// Remove orphaned worktrees left behind by dead workers.
///
/// Lists all git worktrees, compares against live workers, and removes
/// any non-main worktree that no live worker owns.
pub fn gc() -> Result<()> {
    let project_root = std::env::current_dir()?;

    let worktrees = worktree::list_worktrees(&project_root).context("failed to list worktrees")?;

    let live_workers = worker_state::read_all(&project_root)?;
    let live_branches: HashSet<&str> = live_workers.iter().map(|w| w.branch.as_str()).collect();

    let orphaned: Vec<_> = worktrees
        .iter()
        .filter(|wt| !wt.is_main)
        .filter(|wt| {
            wt.branch
                .as_deref()
                .is_none_or(|b| !live_branches.contains(b))
        })
        .collect();

    if orphaned.is_empty() {
        println!("No orphaned worktrees.");
        return Ok(());
    }

    println!("Removing {} orphaned worktree(s):\n", orphaned.len());

    let mut removed = 0;
    for wt in &orphaned {
        let label = wt.branch.as_deref().unwrap_or("(detached)");
        print!("  {} ({})", label, wt.path.display());

        match worktree::remove(&wt.path) {
            Ok(()) => {
                println!(" — removed");
                removed += 1;
            }
            Err(e) => {
                println!(" — failed: {e}");
            }
        }
    }

    if removed > 0 {
        println!("\nRemoved {removed} worktree(s).");
    }

    Ok(())
}
