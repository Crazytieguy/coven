Issue: [P1] VCR test infrastructure: git repo initialization — when the VCR recorder creates a temp dir, also `git init` and create an initial commit so tests are realistic. Re-record all VCR fixtures afterward and update snapshots.
Status: draft

## Approach

### Why this matters

Claude Code's behavior changes when it detects a git repo (e.g., it may use `git diff`, check `.gitignore`, reference recent commits). Recording VCR fixtures in a plain directory produces unrealistic sessions. Initializing a git repo with an initial commit makes the test environment match real usage.

### Changes

#### 1. Add git init + initial commit in `record_vcr.rs`

In `src/bin/record_vcr.rs`, after creating the temp directory and populating files from `case.files` (around line 69), add:

```rust
// Initialize git repo with initial commit
std::process::Command::new("git")
    .args(["init"])
    .current_dir(&tmp_dir)
    .output()?;

std::process::Command::new("git")
    .args(["add", "."])
    .current_dir(&tmp_dir)
    .output()?;

std::process::Command::new("git")
    .args(["commit", "-m", "initial"])
    .current_dir(&tmp_dir)
    .env("GIT_AUTHOR_NAME", "test")
    .env("GIT_AUTHOR_EMAIL", "test@test.com")
    .env("GIT_COMMITTER_NAME", "test")
    .env("GIT_COMMITTER_EMAIL", "test@test.com")
    .output()?;
```

This goes after file creation (so all files from `[files]` are included in the initial commit) and before the `.claude/settings.json` creation (which shouldn't be committed — it's a runtime artifact).

Wait — actually `.claude/settings.json` is created at lines 71-78, before the claude process is spawned. It should probably NOT be in the git repo either (it's coven's test harness artifact). So the order should be:

1. Create temp dir
2. Populate `case.files`
3. `git init` + `git add .` + `git commit`
4. Create `.claude/settings.json` (after commit, so it's untracked)

This way the git repo has a clean initial commit with just the test files, and `.claude/settings.json` is an untracked file that won't affect Claude's behavior.

#### 2. Re-record all VCR fixtures

Run `cargo run --bin record-vcr` to re-record all fixtures with the new git-initialized temp directories.

#### 3. Update snapshots

Run `cargo test` to see snapshot diffs, then `cargo insta accept` to accept the new snapshots. Review the diffs to ensure they look correct — the main expected change is that Claude may now reference git status or behave slightly differently knowing it's in a repo.

### Risk

Claude's responses are nondeterministic. Re-recording all fixtures means all snapshots may change, including in ways unrelated to the git init change. This is expected and acceptable — the snapshots will simply reflect current Claude behavior in a more realistic environment.

## Questions

### Should `.claude/settings.json` be committed or left untracked?

Currently it's created after files but before claude is spawned. With git init, we have a choice:

- **Untracked (recommended):** Create it after the initial commit. It's a test harness artifact, not a "real" project file. Keeping it untracked avoids polluting the repo and is more realistic (users don't typically commit `.claude/settings.json`).
- **Committed:** Include it in the initial commit. Simpler (no ordering concern) but less realistic.

I'd go with untracked — create it after the commit.

Answer:

## Review

