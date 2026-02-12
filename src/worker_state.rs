//! Worker state tracking.
//!
//! Worker state files live in `<git-common-dir>/coven/workers/<branch>.json`.
//!
//! These files are in the shared git directory (not the worktree) so all
//! worktrees can access them. The git common dir is resolved via
//! `git rev-parse --git-common-dir`, which works from any worktree.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// State of a single worker, serialized to JSON.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerState {
    pub pid: u32,
    pub branch: String,
    pub agent: Option<String>,
    pub args: HashMap<String, String>,
}

// ── Path helpers ────────────────────────────────────────────────────────

/// Resolve the shared coven directory: `<git-common-dir>/coven/`.
pub(crate) fn coven_dir(repo_path: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .context("failed to run git rev-parse --git-common-dir")?;

    if !output.status.success() {
        anyhow::bail!("git rev-parse --git-common-dir failed");
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let git_dir = if Path::new(&raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        repo_path.join(raw)
    };

    Ok(git_dir.join("coven"))
}

fn workers_dir(repo_path: &Path) -> Result<PathBuf> {
    Ok(coven_dir(repo_path)?.join("workers"))
}

fn state_file_path(repo_path: &Path, branch: &str) -> Result<PathBuf> {
    Ok(workers_dir(repo_path)?.join(format!("{branch}.json")))
}

// ── Public API ──────────────────────────────────────────────────────────

/// Register this worker by creating its state file.
pub fn register(repo_path: &Path, branch: &str) -> Result<()> {
    let dir = workers_dir(repo_path)?;
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;

    let state = WorkerState {
        pid: std::process::id(),
        branch: branch.to_string(),
        agent: None,
        args: HashMap::new(),
    };

    write_state(repo_path, &state)
}

/// Update this worker's current agent and arguments.
pub fn update<S: std::hash::BuildHasher>(
    repo_path: &Path,
    branch: &str,
    agent: Option<&str>,
    args: &HashMap<String, String, S>,
) -> Result<()> {
    let state = WorkerState {
        pid: std::process::id(),
        branch: branch.to_string(),
        agent: agent.map(String::from),
        args: args.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
    };
    write_state(repo_path, &state)
}

/// Deregister this worker by removing its state file.
pub fn deregister(repo_path: &Path, branch: &str) {
    if let Ok(path) = state_file_path(repo_path, branch) {
        let _ = fs::remove_file(path);
    }
}

/// Read all live worker states, cleaning up stale entries (dead PIDs).
pub fn read_all(repo_path: &Path) -> Result<Vec<WorkerState>> {
    let dir = workers_dir(repo_path)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut states = Vec::new();
    for entry in fs::read_dir(&dir).context("failed to read workers directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json") {
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(state) = serde_json::from_str::<WorkerState>(&content) else {
                let _ = fs::remove_file(&path);
                continue;
            };

            if is_pid_alive(state.pid) {
                states.push(state);
            } else {
                let _ = fs::remove_file(&path);
            }
        }
    }

    Ok(states)
}

/// Style variants for worker status formatting.
#[derive(Clone, Copy)]
pub enum StatusStyle {
    /// CLI status command: indented, em-dash separator, bare agent name.
    Cli,
    /// Dispatch prompt injection: list items, colon separator, "running" prefix.
    Dispatch,
}

/// Format worker states into a multi-line string.
pub fn format_workers<S: Borrow<WorkerState>>(states: &[S], style: StatusStyle) -> String {
    let (line_prefix, separator, agent_prefix) = match style {
        StatusStyle::Cli => ("  ", " — ", ""),
        StatusStyle::Dispatch => ("- ", ": ", "running "),
    };

    let mut out = String::new();
    for item in states {
        let state = item.borrow();
        match &state.agent {
            Some(agent) => {
                let mut args_parts: Vec<_> =
                    state.args.iter().map(|(k, v)| format!("{k}={v}")).collect();
                args_parts.sort();
                if args_parts.is_empty() {
                    let _ = writeln!(
                        out,
                        "{line_prefix}{} (PID {}){separator}{agent_prefix}{agent}",
                        state.branch, state.pid
                    );
                } else {
                    let args_str = args_parts.join(", ");
                    let _ = writeln!(
                        out,
                        "{line_prefix}{} (PID {}){separator}{agent_prefix}{agent} ({args_str})",
                        state.branch, state.pid
                    );
                }
            }
            None => {
                let _ = writeln!(
                    out,
                    "{line_prefix}{} (PID {}){separator}idle",
                    state.branch, state.pid
                );
            }
        }
    }

    out
}

/// Format worker status for injection into the dispatch prompt.
///
/// Excludes the current process (the worker calling dispatch doesn't need
/// to see itself in the status list).
pub fn format_status(states: &[WorkerState], own_branch: &str) -> String {
    let others: Vec<_> = states.iter().filter(|s| s.branch != own_branch).collect();

    if others.is_empty() {
        return "No other workers active.".to_string();
    }

    format_workers(&others, StatusStyle::Dispatch)
}

// ── Private helpers ─────────────────────────────────────────────────────

fn write_state(repo_path: &Path, state: &WorkerState) -> Result<()> {
    let path = state_file_path(repo_path, &state.branch)?;
    let json = serde_json::to_string(state).context("failed to serialize worker state")?;
    fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

/// Check if a process with the given PID is alive.
fn is_pid_alive(pid: u32) -> bool {
    // SAFETY: kill with signal 0 performs error checking without sending a signal.
    unsafe { libc::kill(pid.cast_signed(), 0) == 0 }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn init_repo(dir: &Path) {
        let run = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(args)
                .output()
                .unwrap_or_else(|e| panic!("git {args:?} failed: {e}"));
        };
        run(&["init"]);
        run(&["config", "user.email", "test@test.com"]);
        run(&["config", "user.name", "Test"]);
        fs::write(dir.join("README.md"), "# test\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "init"]);
    }

    #[test]
    fn register_creates_state_file() {
        let repo = TempDir::new().unwrap();
        init_repo(repo.path());

        register(repo.path(), "swift-fox-42").unwrap();

        let path = state_file_path(repo.path(), "swift-fox-42").unwrap();
        assert!(path.exists());

        let content = fs::read_to_string(&path).unwrap();
        let state: WorkerState = serde_json::from_str(&content).unwrap();
        assert_eq!(state.pid, std::process::id());
        assert_eq!(state.branch, "swift-fox-42");
        assert!(state.agent.is_none());
    }

    #[test]
    fn update_changes_state() {
        let repo = TempDir::new().unwrap();
        init_repo(repo.path());

        register(repo.path(), "swift-fox-42").unwrap();

        let args = HashMap::from([("issue".to_string(), "issues/foo.md".to_string())]);
        update(repo.path(), "swift-fox-42", Some("plan"), &args).unwrap();

        let path = state_file_path(repo.path(), "swift-fox-42").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let state: WorkerState = serde_json::from_str(&content).unwrap();
        assert_eq!(state.branch, "swift-fox-42");
        assert_eq!(state.agent.as_deref(), Some("plan"));
        assert_eq!(
            state.args.get("issue").map(String::as_str),
            Some("issues/foo.md")
        );
    }

    #[test]
    fn deregister_removes_file() {
        let repo = TempDir::new().unwrap();
        init_repo(repo.path());

        register(repo.path(), "test-branch").unwrap();
        let path = state_file_path(repo.path(), "test-branch").unwrap();
        assert!(path.exists());

        deregister(repo.path(), "test-branch");
        assert!(!path.exists());
    }

    #[test]
    fn read_all_returns_live_workers() {
        let repo = TempDir::new().unwrap();
        init_repo(repo.path());

        register(repo.path(), "test-branch").unwrap();
        let states = read_all(repo.path()).unwrap();
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].pid, std::process::id());
        assert_eq!(states[0].branch, "test-branch");
    }

    #[test]
    fn read_all_cleans_stale_workers() {
        let repo = TempDir::new().unwrap();
        init_repo(repo.path());

        // Write a state file for a dead PID
        let dir = workers_dir(repo.path()).unwrap();
        fs::create_dir_all(&dir).unwrap();
        let stale = WorkerState {
            pid: 4_000_000_000, // Extremely unlikely to be alive
            branch: "stale-branch".into(),
            agent: Some("plan".into()),
            args: HashMap::new(),
        };
        let stale_path = dir.join("stale-branch.json");
        fs::write(
            &stale_path,
            serde_json::to_string(&stale).unwrap_or_default(),
        )
        .unwrap();

        let states = read_all(repo.path()).unwrap();
        assert!(!states.iter().any(|s| s.pid == 4_000_000_000));
        assert!(!stale_path.exists());
    }

    #[test]
    fn format_status_no_others() {
        let status = format_status(
            &[WorkerState {
                pid: std::process::id(),
                branch: "my-branch".into(),
                agent: Some("plan".into()),
                args: HashMap::new(),
            }],
            "my-branch",
        );
        assert_eq!(status, "No other workers active.");
    }

    #[test]
    fn format_status_with_others() {
        let states = vec![
            WorkerState {
                pid: std::process::id(),
                branch: "my-branch".into(),
                agent: None,
                args: HashMap::new(),
            },
            WorkerState {
                pid: 12345,
                branch: "swift-fox-42".into(),
                agent: Some("implement".into()),
                args: HashMap::from([("issue".into(), "issues/foo.md".into())]),
            },
            WorkerState {
                pid: 12346,
                branch: "bold-oak-7".into(),
                agent: None,
                args: HashMap::new(),
            },
        ];
        let formatted = format_status(&states, "my-branch");
        assert!(
            formatted.contains("swift-fox-42 (PID 12345): running implement (issue=issues/foo.md)")
        );
        assert!(formatted.contains("bold-oak-7 (PID 12346): idle"));
        assert!(!formatted.contains("my-branch"));
    }

    #[test]
    fn format_workers_cli_style() {
        let states = vec![
            WorkerState {
                pid: 12345,
                branch: "swift-fox-42".into(),
                agent: Some("implement".into()),
                args: HashMap::from([("issue".into(), "issues/foo.md".into())]),
            },
            WorkerState {
                pid: 12346,
                branch: "bold-oak-7".into(),
                agent: None,
                args: HashMap::new(),
            },
        ];
        let formatted = format_workers(&states, StatusStyle::Cli);
        assert!(formatted.contains("  swift-fox-42 (PID 12345) — implement (issue=issues/foo.md)"));
        assert!(formatted.contains("  bold-oak-7 (PID 12346) — idle"));
    }
}
