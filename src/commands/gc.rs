use std::collections::HashSet;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

use crate::vcr::VcrContext;
use crate::worker_state;
use crate::worktree;

/// Remove orphaned worktrees left behind by dead workers.
///
/// Lists all git worktrees, compares against live workers, and removes
/// any non-main worktree that no live worker owns.
pub async fn gc(
    vcr: &VcrContext,
    working_dir: Option<&Path>,
    writer: &mut impl Write,
) -> Result<()> {
    let project_root = super::resolve_working_dir(vcr, working_dir).await?;

    let worktrees = vcr
        .call(
            "worktree::list_worktrees",
            project_root.clone(),
            async |p: &String| {
                worktree::list_worktrees(Path::new(p)).map_err(|e| anyhow::anyhow!("{e}"))
            },
        )
        .await
        .context("failed to list worktrees")?;

    let live_workers = vcr
        .call(
            "worker_state::read_all",
            project_root,
            async |p: &String| worker_state::read_all(Path::new(p)),
        )
        .await?;

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
        writeln!(writer, "No orphaned worktrees.")?;
        return Ok(());
    }

    writeln!(
        writer,
        "Removing {} orphaned worktree(s):\n",
        orphaned.len()
    )?;

    let mut removed = 0;
    for wt in &orphaned {
        let label = wt.branch.as_deref().unwrap_or("(detached)");
        write!(writer, "  {} ({})", label, wt.path.display())?;

        let wt_path = wt.path.display().to_string();
        let result = vcr
            .call("worktree::remove", wt_path, async |p: &String| {
                worktree::remove(Path::new(p)).map_err(|e| anyhow::anyhow!("{e}"))
            })
            .await;

        match result {
            Ok(()) => {
                writeln!(writer, " — removed")?;
                removed += 1;
            }
            Err(e) => {
                writeln!(writer, " — failed: {e}")?;
            }
        }
    }

    if removed > 0 {
        writeln!(writer, "\nRemoved {removed} worktree(s).")?;
    }

    Ok(())
}
