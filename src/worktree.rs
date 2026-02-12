use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rand::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, thiserror::Error)]
pub enum WorktreeError {
    #[error("not a git repository")]
    NotGitRepo,
    #[error("branch '{0}' already exists")]
    BranchExists(String),
    #[error("worktree has uncommitted changes")]
    DirtyWorkingTree,
    #[error("worktree has untracked files")]
    UntrackedFiles,
    #[error("cannot land from the main worktree")]
    IsMainWorktree,
    #[error("detached HEAD state")]
    DetachedHead,
    #[error("rebase conflict in: {0:?}")]
    RebaseConflict(Vec<String>),
    #[error("fast-forward failed — main has diverged")]
    FastForwardFailed,
    #[error("git command failed: {0}")]
    GitCommand(String),
}

/// Configuration for spawn operations.
pub struct SpawnOptions<'a> {
    /// Path to the git repo (or any worktree of it).
    pub repo_path: &'a Path,
    /// Optional branch name. If None, a random adjective-noun-N name is generated.
    pub branch: Option<&'a str>,
    /// Base directory for worktrees. Worktree will be created at `<base>/<project>/<branch>/`.
    pub base_path: &'a Path,
}

/// Result of a successful spawn operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct SpawnResult {
    pub worktree_path: PathBuf,
    pub branch: String,
}

/// Result of a successful land operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct LandResult {
    pub branch: String,
    pub main_branch: String,
}

// ── Word lists for random branch names ──────────────────────────────────

const ADJECTIVES: &[&str] = &[
    "swift", "quick", "bright", "calm", "clever", "cool", "crisp", "eager", "fast", "fresh",
    "keen", "light", "neat", "prime", "sharp", "silent", "smooth", "steady", "warm", "bold",
    "brave", "clear", "fleet", "golden", "agile", "nimble", "rapid", "blazing", "cosmic",
];

const NOUNS: &[&str] = &[
    "fox", "wolf", "bear", "hawk", "lion", "tiger", "raven", "eagle", "falcon", "otter", "cedar",
    "maple", "oak", "pine", "willow", "river", "stream", "brook", "delta", "canyon", "spark",
    "flame", "ember", "comet", "meteor", "nova", "pulse", "wave", "drift", "glow",
];

// ── Internal helpers ────────────────────────────────────────────────────

fn path_str(path: &Path) -> Result<&str, WorktreeError> {
    path.to_str()
        .ok_or_else(|| WorktreeError::GitCommand("path is not valid UTF-8".into()))
}

/// Run a git command in the given directory and return stdout.
fn git(dir: &Path, args: &[&str]) -> Result<String, WorktreeError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| WorktreeError::GitCommand(format!("failed to run git: {e}")))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(WorktreeError::GitCommand(format!(
            "git {} failed: {}",
            args.join(" "),
            stderr.trim()
        )))
    }
}

/// Run a git command and return whether it exited successfully (ignoring output).
fn git_status(dir: &Path, args: &[&str]) -> Result<bool, WorktreeError> {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| WorktreeError::GitCommand(format!("failed to run git: {e}")))?;
    Ok(status.success())
}

/// Parse `git worktree list --porcelain` to find the main worktree path and branch.
fn find_main_worktree(repo: &Path) -> Result<(PathBuf, String), WorktreeError> {
    let output = git(repo, &["worktree", "list", "--porcelain"])?;

    let mut path = None;
    let mut branch = None;

    for line in output.lines() {
        if path.is_none()
            && let Some(p) = line.strip_prefix("worktree ")
        {
            path = Some(PathBuf::from(p));
        }
        if branch.is_none()
            && let Some(b) = line.strip_prefix("branch refs/heads/")
        {
            branch = Some(b.to_string());
        }
        if line.is_empty() {
            break; // Only parse the first worktree entry
        }
    }

    match (path, branch) {
        (Some(p), Some(b)) => Ok((p, b)),
        _ => Err(WorktreeError::GitCommand(
            "could not parse worktree list output".into(),
        )),
    }
}

fn generate_branch_name() -> String {
    let mut rng = rand::rng();
    let adj = ADJECTIVES.choose(&mut rng).copied().unwrap_or("swift");
    let noun = NOUNS.choose(&mut rng).copied().unwrap_or("fox");
    let num: u32 = rng.random_range(0..100);
    format!("{adj}-{noun}-{num}")
}

/// Resolve the git common directory for a repository or worktree.
///
/// Runs `git rev-parse --git-common-dir` and normalizes the result to an
/// absolute path (the command may return a relative path in some setups).
pub fn git_common_dir(repo_path: &Path) -> Result<PathBuf, WorktreeError> {
    let raw = git(repo_path, &["rev-parse", "--git-common-dir"])?;
    let trimmed = raw.trim();
    Ok(if Path::new(trimmed).is_absolute() {
        PathBuf::from(trimmed)
    } else {
        repo_path.join(trimmed)
    })
}

// ── Public API ──────────────────────────────────────────────────────────

/// Get the main branch name by parsing `git worktree list --porcelain`.
pub fn main_branch_name(repo: &Path) -> Result<String, WorktreeError> {
    let (_, branch) = find_main_worktree(repo)?;
    Ok(branch)
}

/// A git worktree entry from `git worktree list --porcelain`.
#[derive(Serialize, Deserialize)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    /// Branch name (without refs/heads/ prefix). None for detached HEAD.
    pub branch: Option<String>,
    /// Whether this is the main worktree (first entry in the list).
    pub is_main: bool,
}

/// List all worktrees in the repository.
pub fn list_worktrees(repo_path: &Path) -> Result<Vec<WorktreeEntry>, WorktreeError> {
    let output = git(repo_path, &["worktree", "list", "--porcelain"])?;

    let mut entries = Vec::new();
    let mut current_path = None;
    let mut current_branch = None;

    for line in output.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            current_path = Some(PathBuf::from(p));
        } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
            current_branch = Some(b.to_string());
        } else if line.is_empty() {
            if let Some(path) = current_path.take() {
                let is_main = entries.is_empty();
                entries.push(WorktreeEntry {
                    path,
                    branch: current_branch.take(),
                    is_main,
                });
            }
            current_branch = None;
        }
    }
    // Handle last entry (porcelain output may not have trailing blank line)
    if let Some(path) = current_path {
        let is_main = entries.is_empty();
        entries.push(WorktreeEntry {
            path,
            branch: current_branch,
            is_main,
        });
    }

    Ok(entries)
}

/// Spawn a new worktree with a random branch name (or caller-provided name).
///
/// - Validates we're in a git repo
/// - Generates a random adjective-noun-N branch name if none provided
/// - Runs `git worktree add -b <branch> <path>`
/// - Rsyncs gitignored files from main repo to worktree
/// - Worktree location: `<base_path>/<project>/<branch>/`
pub fn spawn(options: &SpawnOptions<'_>) -> Result<SpawnResult, WorktreeError> {
    // Validate git repo
    if !git_status(options.repo_path, &["rev-parse", "--git-dir"])? {
        return Err(WorktreeError::NotGitRepo);
    }

    let branch = match options.branch {
        Some(b) => b.to_string(),
        None => generate_branch_name(),
    };

    // Check branch doesn't already exist
    if git_status(
        options.repo_path,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )? {
        return Err(WorktreeError::BranchExists(branch));
    }

    // Find main worktree to get project name
    let (main_path, _) = find_main_worktree(options.repo_path)?;
    let project = main_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| WorktreeError::GitCommand("could not determine project name".into()))?;

    let worktree_path = options.base_path.join(project).join(&branch);

    // Create parent directory
    std::fs::create_dir_all(options.base_path.join(project))
        .map_err(|e| WorktreeError::GitCommand(format!("failed to create directory: {e}")))?;

    // Create worktree with new branch (from the main repo)
    let wt_str = path_str(&worktree_path)?;
    git(&main_path, &["worktree", "add", "-b", &branch, wt_str])?;

    // Copy gitignored files via rsync
    rsync_ignored(&main_path, &worktree_path)?;

    Ok(SpawnResult {
        worktree_path,
        branch,
    })
}

/// Land the worktree's branch onto the main branch.
///
/// - Validates we're in a secondary worktree with clean working tree
/// - Rebases current branch onto main
/// - Fast-forward merges main to current branch tip
///
/// Does NOT remove the worktree — the worktree persists for continued use.
/// Returns an error with conflict details if rebase fails.
pub fn land(worktree_path: &Path) -> Result<LandResult, WorktreeError> {
    let (main_path, main_branch) = find_main_worktree(worktree_path)?;

    // Check we're not in the main worktree
    let toplevel = git(worktree_path, &["rev-parse", "--show-toplevel"])?;
    if main_path == Path::new(toplevel.trim()) {
        return Err(WorktreeError::IsMainWorktree);
    }

    // Check for detached HEAD
    let current_branch = git(worktree_path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let current_branch = current_branch.trim().to_string();
    if current_branch == "HEAD" {
        return Err(WorktreeError::DetachedHead);
    }

    // Check for uncommitted changes or untracked files
    match dirty_state(worktree_path)? {
        DirtyState::Clean => {}
        DirtyState::UncommittedChanges => return Err(WorktreeError::DirtyWorkingTree),
        DirtyState::UntrackedFiles => return Err(WorktreeError::UntrackedFiles),
    }

    // Rebase onto main
    let rebase_output = Command::new("git")
        .arg("-C")
        .arg(worktree_path)
        .args(["rebase", &main_branch])
        .output()
        .map_err(|e| WorktreeError::GitCommand(format!("failed to run git: {e}")))?;

    if !rebase_output.status.success() {
        let conflicts = match git(worktree_path, &["diff", "--name-only", "--diff-filter=U"]) {
            Ok(output) => output,
            Err(diff_err) => {
                let stderr = String::from_utf8_lossy(&rebase_output.stderr);
                return Err(WorktreeError::GitCommand(format!(
                    "rebase failed: {} (and failed to list conflicts: {diff_err})",
                    stderr.trim()
                )));
            }
        };
        let conflict_files: Vec<String> = conflicts
            .lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();

        if conflict_files.is_empty() {
            let stderr = String::from_utf8_lossy(&rebase_output.stderr);
            return Err(WorktreeError::GitCommand(format!(
                "rebase failed: {}",
                stderr.trim()
            )));
        }

        return Err(WorktreeError::RebaseConflict(conflict_files));
    }

    // Fast-forward merge main to current branch tip
    if !git_status(&main_path, &["merge", "--ff-only", &current_branch])? {
        return Err(WorktreeError::FastForwardFailed);
    }

    Ok(LandResult {
        branch: current_branch,
        main_branch,
    })
}

/// Remove a worktree and delete its branch.
///
/// - Runs `git worktree remove [--force] <path>`
/// - Deletes the branch
///
/// When `force` is true, passes `--force` to `git worktree remove` and uses
/// `branch -D` instead of `branch -d`, allowing removal of dirty worktrees.
///
/// Intended for worker shutdown, not after every land.
pub fn remove(worktree_path: &Path, force: bool) -> Result<(), WorktreeError> {
    let branch = git(worktree_path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let branch = branch.trim().to_string();

    let (main_path, _) = find_main_worktree(worktree_path)?;

    let wt_str = path_str(worktree_path)?;
    if force {
        git(&main_path, &["worktree", "remove", "--force", wt_str])?;
    } else {
        git(&main_path, &["worktree", "remove", wt_str])?;
    }

    // Delete the branch (ignore errors — branch may already be gone)
    let delete_flag = if force { "-D" } else { "-d" };
    let _ = git(&main_path, &["branch", delete_flag, &branch]);

    Ok(())
}

/// Update the worktree branch to include the latest commits from main.
///
/// If the worktree has no unique commits (normal state after landing),
/// this is a fast-forward. If the worktree has unique commits, they
/// are rebased onto main.
///
/// Call this before dispatch so the agent sees the latest issue files.
pub fn sync_to_main(worktree_path: &Path) -> Result<(), WorktreeError> {
    let (_, main_branch) = find_main_worktree(worktree_path)?;
    git(worktree_path, &["rebase", &main_branch])?;
    Ok(())
}

/// Reset the worktree branch to main's tip, discarding any local commits.
///
/// Used after a failed land to put the worktree back in a clean state
/// so the next dispatch can start fresh.
pub fn reset_to_main(worktree_path: &Path) -> Result<(), WorktreeError> {
    let (_, main_branch) = find_main_worktree(worktree_path)?;
    git(worktree_path, &["reset", "--hard", &main_branch])?;
    Ok(())
}

/// Abort a failed rebase in the given worktree.
pub fn abort_rebase(worktree_path: &Path) -> Result<(), WorktreeError> {
    git(worktree_path, &["rebase", "--abort"])?;
    Ok(())
}

/// Remove untracked, non-ignored files and directories from the worktree.
///
/// Runs `git clean -fd`. Gitignored files (build artifacts, etc.) are preserved.
/// Used during land failure recovery to prevent stray files from blocking
/// future land attempts.
pub fn clean(worktree_path: &Path) -> Result<(), WorktreeError> {
    git(worktree_path, &["clean", "-fd"])?;
    Ok(())
}

/// Check whether the worktree branch has any commits ahead of main.
pub fn has_unique_commits(worktree_path: &Path) -> Result<bool, WorktreeError> {
    let (_, main_branch) = find_main_worktree(worktree_path)?;
    let output = git(
        worktree_path,
        &["rev-list", "--count", &format!("{main_branch}..HEAD")],
    )?;
    let count: u64 = output
        .trim()
        .parse()
        .map_err(|e| WorktreeError::GitCommand(format!("failed to parse rev-list count: {e}")))?;
    Ok(count > 0)
}

/// What kind of dirt the worktree has.
#[derive(Debug, Serialize, Deserialize)]
pub enum DirtyState {
    Clean,
    /// Staged or unstaged modifications/deletions.
    UncommittedChanges,
    /// Untracked, non-ignored files.
    UntrackedFiles,
}

/// Check the worktree for uncommitted changes or untracked files.
///
/// Returns the first kind of dirt found (uncommitted changes take priority
/// over untracked files since `git clean -fd` handles the latter).
pub fn dirty_state(worktree_path: &Path) -> Result<DirtyState, WorktreeError> {
    // Unstaged changes
    if !git_status(worktree_path, &["diff", "--quiet"])? {
        return Ok(DirtyState::UncommittedChanges);
    }
    // Staged but uncommitted changes
    if !git_status(worktree_path, &["diff", "--cached", "--quiet"])? {
        return Ok(DirtyState::UncommittedChanges);
    }
    // Untracked files
    let untracked = git(
        worktree_path,
        &["ls-files", "--others", "--exclude-standard"],
    )?;
    if !untracked.trim().is_empty() {
        return Ok(DirtyState::UntrackedFiles);
    }
    Ok(DirtyState::Clean)
}

/// Check if a rebase is currently in progress in the worktree.
pub fn is_rebase_in_progress(worktree_path: &Path) -> Result<bool, WorktreeError> {
    let git_dir_output = git(worktree_path, &["rev-parse", "--git-dir"])?;
    let git_dir = PathBuf::from(git_dir_output.trim());
    Ok(git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists())
}

// ── Private helpers ─────────────────────────────────────────────────────

fn rsync_ignored(main_path: &Path, worktree_path: &Path) -> Result<(), WorktreeError> {
    let ignored = git(
        main_path,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
        ],
    )?;

    if ignored.trim().is_empty() {
        return Ok(());
    }

    let mut child = Command::new("rsync")
        .arg("-a")
        .arg("--files-from=-")
        .arg(format!("{}/", main_path.display()))
        .arg(format!("{}/", worktree_path.display()))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| WorktreeError::GitCommand(format!("failed to run rsync: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        // Write and drop to signal EOF; ignore broken pipe (some files may not exist)
        let _ = stdin.write_all(ignored.as_bytes());
    }

    // Non-fatal: rsync may warn about missing gitignored files
    let _ = child.wait();

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Initialize a git repo with an initial commit.
    fn init_repo(dir: &Path) {
        git(dir, &["init"]).unwrap();
        git(dir, &["config", "user.email", "test@test.com"]).unwrap();
        git(dir, &["config", "user.name", "Test"]).unwrap();
        fs::write(dir.join("README.md"), "# test repo\n").unwrap();
        git(dir, &["add", "."]).unwrap();
        git(dir, &["commit", "-m", "initial commit"]).unwrap();
    }

    /// Create a file, add, and commit.
    fn commit_file(dir: &Path, name: &str, content: &str, message: &str) {
        let file_path = dir.join(name);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&file_path, content).unwrap();
        git(dir, &["add", name]).unwrap();
        git(dir, &["commit", "-m", message]).unwrap();
    }

    fn spawn_opts<'a>(repo: &'a Path, base: &'a Path, branch: Option<&'a str>) -> SpawnOptions<'a> {
        SpawnOptions {
            repo_path: repo,
            branch,
            base_path: base,
        }
    }

    #[test]
    fn spawn_creates_worktree() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let result = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("test-branch"),
        ));
        let result = result.unwrap();

        assert_eq!(result.branch, "test-branch");
        assert!(result.worktree_path.exists());
        assert!(result.worktree_path.join("README.md").exists());
    }

    #[test]
    fn spawn_copies_gitignored_files() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        // Create a .gitignore and an ignored file
        fs::write(repo_dir.path().join(".gitignore"), "build/\n").unwrap();
        git(repo_dir.path(), &["add", ".gitignore"]).unwrap();
        git(repo_dir.path(), &["commit", "-m", "add gitignore"]).unwrap();

        fs::create_dir_all(repo_dir.path().join("build")).unwrap();
        fs::write(repo_dir.path().join("build/output.txt"), "compiled stuff\n").unwrap();

        let result = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("wt-ignored"),
        ));
        let result = result.unwrap();

        assert!(result.worktree_path.join("build/output.txt").exists());
    }

    #[test]
    fn spawn_custom_branch_name() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let result = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("my-feature"),
        ));
        let result = result.unwrap();

        assert_eq!(result.branch, "my-feature");
        assert!(result.worktree_path.ends_with("my-feature"));
    }

    #[test]
    fn spawn_duplicate_branch_errors() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        // Create a branch
        git(repo_dir.path(), &["branch", "existing-branch"]).unwrap();

        let result = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("existing-branch"),
        ));

        assert!(
            matches!(result, Err(WorktreeError::BranchExists(ref b)) if b == "existing-branch")
        );
    }

    #[test]
    fn land_clean_rebase() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("feature"),
        ));
        let spawned = spawned.unwrap();

        // Commit in the worktree
        commit_file(&spawned.worktree_path, "new.txt", "hello\n", "add new file");

        // Land
        let landed = land(&spawned.worktree_path).unwrap();
        assert_eq!(landed.branch, "feature");

        // Verify main has the commit
        let log = git(repo_dir.path(), &["log", "--oneline"]).unwrap();
        assert!(log.contains("add new file"));

        // Verify worktree still exists
        assert!(spawned.worktree_path.exists());
    }

    #[test]
    fn land_with_conflict() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("conflict-branch"),
        ));
        let spawned = spawned.unwrap();

        // Commit conflicting change on main
        commit_file(repo_dir.path(), "file.txt", "main content\n", "main change");

        // Commit conflicting change in worktree
        commit_file(
            &spawned.worktree_path,
            "file.txt",
            "worktree content\n",
            "worktree change",
        );

        let result = land(&spawned.worktree_path);
        assert!(
            matches!(result, Err(WorktreeError::RebaseConflict(ref files)) if files.contains(&"file.txt".to_string()))
        );

        // Clean up the rebase state
        abort_rebase(&spawned.worktree_path).unwrap();
    }

    #[test]
    fn land_dirty_worktree_errors() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("dirty-branch"),
        ));
        let spawned = spawned.unwrap();

        // Modify a file without committing
        fs::write(spawned.worktree_path.join("README.md"), "modified\n").unwrap();

        let result = land(&spawned.worktree_path);
        assert!(matches!(result, Err(WorktreeError::DirtyWorkingTree)));
    }

    #[test]
    fn abort_rebase_restores_clean_state() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("abort-branch"),
        ));
        let spawned = spawned.unwrap();

        // Create a conflict
        commit_file(repo_dir.path(), "conflict.txt", "main\n", "main side");
        commit_file(
            &spawned.worktree_path,
            "conflict.txt",
            "worktree\n",
            "wt side",
        );

        let result = land(&spawned.worktree_path);
        assert!(matches!(result, Err(WorktreeError::RebaseConflict(_))));

        // Abort the rebase
        abort_rebase(&spawned.worktree_path).unwrap();

        // Verify clean state — diff should be quiet
        assert!(git_status(&spawned.worktree_path, &["diff", "--quiet"]).unwrap());
    }

    #[test]
    fn sync_to_main_picks_up_new_commits() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("sync-branch"),
        ))
        .unwrap();

        // Commit on main after the worktree was spawned
        commit_file(
            repo_dir.path(),
            "new-on-main.txt",
            "from main\n",
            "main commit",
        );

        // Worktree doesn't have the file yet
        assert!(!spawned.worktree_path.join("new-on-main.txt").exists());

        // Sync picks it up
        sync_to_main(&spawned.worktree_path).unwrap();
        assert!(spawned.worktree_path.join("new-on-main.txt").exists());
    }

    #[test]
    fn sync_to_main_noop_when_up_to_date() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("sync-noop"),
        ))
        .unwrap();

        // Sync when already up to date should succeed
        sync_to_main(&spawned.worktree_path).unwrap();
    }

    #[test]
    fn reset_to_main_discards_local_commits() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("reset-branch"),
        ))
        .unwrap();

        // Make a commit in the worktree
        commit_file(
            &spawned.worktree_path,
            "local.txt",
            "local\n",
            "local commit",
        );
        assert!(spawned.worktree_path.join("local.txt").exists());

        // Reset to main
        reset_to_main(&spawned.worktree_path).unwrap();

        // Local file should be gone
        assert!(!spawned.worktree_path.join("local.txt").exists());
    }

    #[test]
    fn reset_to_main_after_conflict_abort() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("reset-conflict"),
        ))
        .unwrap();

        // Create a conflict
        commit_file(repo_dir.path(), "file.txt", "main\n", "main side");
        commit_file(&spawned.worktree_path, "file.txt", "worktree\n", "wt side");

        // Land fails with conflict
        let result = land(&spawned.worktree_path);
        assert!(matches!(result, Err(WorktreeError::RebaseConflict(_))));

        // Abort rebase, then reset to main
        abort_rebase(&spawned.worktree_path).unwrap();
        reset_to_main(&spawned.worktree_path).unwrap();

        // Worktree should now have main's version
        let content = fs::read_to_string(spawned.worktree_path.join("file.txt")).unwrap();
        assert_eq!(content, "main\n");
    }

    #[test]
    fn clean_removes_untracked_files() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("clean-branch"),
        ))
        .unwrap();

        // Create untracked files
        fs::write(spawned.worktree_path.join("stray.txt"), "leftover\n").unwrap();
        fs::create_dir_all(spawned.worktree_path.join("stray-dir")).unwrap();
        fs::write(
            spawned.worktree_path.join("stray-dir/nested.txt"),
            "nested\n",
        )
        .unwrap();

        assert!(spawned.worktree_path.join("stray.txt").exists());
        assert!(spawned.worktree_path.join("stray-dir/nested.txt").exists());

        clean(&spawned.worktree_path).unwrap();

        assert!(!spawned.worktree_path.join("stray.txt").exists());
        assert!(!spawned.worktree_path.join("stray-dir").exists());
        // Tracked files should still be there
        assert!(spawned.worktree_path.join("README.md").exists());
    }

    #[test]
    fn clean_preserves_gitignored_files() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        // Add a .gitignore
        fs::write(repo_dir.path().join(".gitignore"), "build/\n").unwrap();
        git(repo_dir.path(), &["add", ".gitignore"]).unwrap();
        git(repo_dir.path(), &["commit", "-m", "add gitignore"]).unwrap();

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("clean-ignore"),
        ))
        .unwrap();

        // Create an ignored directory and an untracked file
        fs::create_dir_all(spawned.worktree_path.join("build")).unwrap();
        fs::write(spawned.worktree_path.join("build/output.bin"), "binary\n").unwrap();
        fs::write(spawned.worktree_path.join("stray.txt"), "leftover\n").unwrap();

        clean(&spawned.worktree_path).unwrap();

        // Untracked file should be removed
        assert!(!spawned.worktree_path.join("stray.txt").exists());
        // Gitignored file should be preserved
        assert!(spawned.worktree_path.join("build/output.bin").exists());
    }

    #[test]
    fn is_rebase_in_progress_false_normally() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("no-rebase"),
        ))
        .unwrap();

        assert!(!is_rebase_in_progress(&spawned.worktree_path).unwrap());
    }

    #[test]
    fn has_unique_commits_true_when_ahead() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("unique-commits"),
        ))
        .unwrap();

        // No unique commits initially
        assert!(!has_unique_commits(&spawned.worktree_path).unwrap());

        // Make a commit in the worktree
        commit_file(&spawned.worktree_path, "new.txt", "hello\n", "add file");

        // Now has unique commits
        assert!(has_unique_commits(&spawned.worktree_path).unwrap());
    }

    #[test]
    fn has_unique_commits_false_after_land() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("unique-land"),
        ))
        .unwrap();

        commit_file(&spawned.worktree_path, "new.txt", "hello\n", "add file");
        assert!(has_unique_commits(&spawned.worktree_path).unwrap());

        land(&spawned.worktree_path).unwrap();

        // After landing, worktree branch and main are at the same tip
        assert!(!has_unique_commits(&spawned.worktree_path).unwrap());
    }

    #[test]
    fn remove_worktree() {
        let repo_dir = TempDir::new().unwrap();
        let base_dir = TempDir::new().unwrap();
        init_repo(repo_dir.path());

        let spawned = spawn(&spawn_opts(
            repo_dir.path(),
            base_dir.path(),
            Some("rm-branch"),
        ));
        let spawned = spawned.unwrap();

        assert!(spawned.worktree_path.exists());

        remove(&spawned.worktree_path, false).unwrap();

        // Directory should be gone
        assert!(!spawned.worktree_path.exists());

        // Branch should be gone
        let branch_check = git_status(
            repo_dir.path(),
            &["show-ref", "--verify", "--quiet", "refs/heads/rm-branch"],
        )
        .unwrap();
        assert!(!branch_check);
    }
}
