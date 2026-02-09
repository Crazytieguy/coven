Issue: [P1] Git worktree primitives — new module wrapping git worktree operations: spawn (random branch, `git worktree add -b`, rsync gitignored files) and land (rebase, conflict detection, ff-merge, branch cleanup). Tested with real git in temp repos.
Status: draft

## Approach

### New module: `src/worktree.rs`

A standalone module with no dependencies on the rest of coven (no display, session, protocol imports). Uses `std::process::Command` to shell out to `git` and `rsync`, matching the design examples.

### Public API

```rust
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
pub struct SpawnResult {
    pub worktree_path: PathBuf,
    pub branch: String,
}

/// Spawn a new worktree with a random branch name (or caller-provided name).
/// - Validates we're in a git repo
/// - Generates a random adjective-noun-N branch name if none provided
/// - Runs `git worktree add -b <branch> <path>`
/// - Rsyncs gitignored files from main repo to worktree
/// - Worktree location: `<base_path>/<project>/<branch>/`
pub fn spawn(options: SpawnOptions<'_>) -> Result<SpawnResult, WorktreeError>;

/// Land the worktree's branch onto the main branch.
/// - Validates we're in a secondary worktree with clean working tree
/// - Rebases current branch onto main
/// - Fast-forward merges main to current branch tip
/// Does NOT remove the worktree — the worktree persists for continued use.
/// Returns an error with conflict details if rebase fails.
pub fn land(worktree_path: &Path) -> Result<LandResult, WorktreeError>;

/// Remove a worktree and delete its branch.
/// - Runs `git worktree remove <path>`
/// - Deletes the branch
/// Intended for worker shutdown, not after every land.
pub fn remove(worktree_path: &Path) -> Result<(), WorktreeError>;

/// Abort a failed rebase in the given worktree.
pub fn abort_rebase(worktree_path: &Path) -> Result<(), WorktreeError>;
```

### Error type

```rust
#[derive(Debug, thiserror::Error)]
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
```

### Random branch names

Port the adjective-noun word lists from the spawn example script. Use `rand` crate (already added to Cargo.toml).

### Internal helpers

- `git(repo: &Path, args: &[&str]) -> Result<String, WorktreeError>` — run a git command in a given directory, return stdout, map non-zero exit to `GitCommand` error.
- `find_main_worktree(repo: &Path) -> Result<(PathBuf, String), WorktreeError>` — parse `git worktree list --porcelain` to find the main worktree path and branch.
- `generate_branch_name() -> String` — random adjective-noun-N.

### Tests

Integration tests in `src/worktree.rs` (as `#[cfg(test)] mod tests`) using real git repos in `tempfile::TempDir`. All tests use a tmp base path for worktree creation.

1. **spawn_creates_worktree** — init a repo, commit a file, spawn a worktree, verify the worktree dir exists and has the file.
2. **spawn_copies_gitignored_files** — create a gitignored file (e.g. `target/`), spawn, verify it exists in the worktree.
3. **spawn_custom_branch_name** — pass an explicit branch name, verify it's used.
4. **spawn_duplicate_branch_errors** — create a branch, try to spawn with same name, expect `BranchExists`.
5. **land_clean_rebase** — spawn, commit in worktree, land, verify main has the commit. Verify worktree still exists after land.
6. **land_with_conflict** — commit conflicting changes in main and worktree, land, expect `RebaseConflict`.
7. **land_dirty_worktree_errors** — modify a file without committing, land, expect `DirtyWorkingTree`.
8. **abort_rebase** — trigger a conflict, abort, verify clean state.
9. **remove_worktree** — spawn a worktree, remove it, verify directory and branch are gone.

### Dependencies

- `thiserror` — already added to Cargo.toml.
- `rand` — already added to Cargo.toml.
- Add `tempfile` as a dev-dependency for tests.

### Wire-up

Register `pub mod worktree;` in `src/lib.rs`. No behavioral integration yet — this is just the primitives module.

## Questions

(No open questions — previous questions about `rand` and `thiserror` are resolved, both already added to Cargo.toml.)

## Review

