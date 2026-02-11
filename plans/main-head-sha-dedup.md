Issue: [P2] `main_head_sha` in worker.rs re-derives the main branch name by parsing `git worktree list --porcelain`, duplicating logic already in `find_main_worktree` in worktree.rs. Extract a shared helper for finding the main branch name.
Status: draft

## Approach

`main_head_sha` (worker.rs:991) runs `git worktree list --porcelain` and parses `branch refs/heads/...` to find the main branch name — identical to what `find_main_worktree` (worktree.rs:110) already does.

**Change:**

1. **worktree.rs** — Add a public function:
   ```rust
   pub fn main_branch_name(repo: &Path) -> Result<String, WorktreeError> {
       let (_, branch) = find_main_worktree(repo)?;
       Ok(branch)
   }
   ```

2. **worker.rs** — Simplify `main_head_sha` to use the new helper:
   ```rust
   fn main_head_sha(worktree_path: &Path) -> Result<String> {
       let main_branch = crate::worktree::main_branch_name(worktree_path)?;
       let output = std::process::Command::new("git")
           .arg("-C")
           .arg(worktree_path)
           .args(["rev-parse", &main_branch])
           .output()
           .context("failed to run git rev-parse")?;
       Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
   }
   ```

The VCR wrapping (`vcr_main_head_sha`) stays identical — it wraps the whole function, so both the worktree-list and rev-parse calls are captured in the same VCR recording. No VCR changes needed.

No re-recording needed either: the VCR captures `main_head_sha`'s input (worktree path) and output (SHA string). The internal implementation changing doesn't affect the recorded values.

## Questions

None — this is a straightforward dedup.

## Review

