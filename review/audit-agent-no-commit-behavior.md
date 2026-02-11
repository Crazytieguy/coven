---
priority: P1
state: review
---

# Audit agent creates low-value issues instead of finding nothing

The audit agent currently always proposes issues. If none of its findings are genuine improvements, it creates low-value issues anyway, which can lead to an infinite audit loop.

## Changes requested

Two changes — one to the prompt, one to the worker's handling of no-commit agents.

### 1. Update audit agent prompt

Keep the planned prompt addition telling audit it's okay to find nothing and not commit. Don't manufacture low-value issues just to have output.

### 2. Configurable no-commit behavior via agent frontmatter

Don't encode agent-specific logic in the CLI. Instead, add a frontmatter field to agent configs (e.g. `on_no_commits: sleep`) that tells the worker what to do when an agent finishes without committing.

After an agent produces no commits, the worker should check the field and:

- **`sleep`** (for audit): Check the working directory state:
  - **Clean** → agent genuinely found nothing → sleep until new commits
  - **Dirty** → agent did work but didn't commit → send a follow-up telling it to commit
- **Default / unset** (for other agents): Current behavior — other agents should always commit

This keeps agent-specific policy in agent config rather than hardcoded in the worker loop.

## Plan

Three files to change, plus tests.

### 1. Add `on_no_commits` to `AgentFrontmatter` (`src/agents.rs`)

Add an optional `on_no_commits` field to `AgentFrontmatter`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OnNoCommits {
    /// Default: resume the session once asking the agent to commit (current behavior).
    #[default]
    Prompt,
    /// Sleep until new commits arrive on main (used by audit).
    /// If the working tree is dirty, still prompt the agent to commit first.
    Sleep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFrontmatter {
    pub description: String,
    #[serde(default)]
    pub args: Vec<AgentArg>,
    #[serde(default)]
    pub on_no_commits: OnNoCommits,
}
```

Add a unit test that parses an agent file with `on_no_commits: sleep` and verifies the field is set correctly, plus one verifying the default is `Prompt` when omitted.

### 2. Update `run_agent` and `ensure_commits` to respect the field (`src/commands/worker.rs`)

**Pass `on_no_commits` into `run_agent`:** The call site in `worker_loop` already has `agent_def` available (line 369). Add an `on_no_commits: &OnNoCommits` parameter to `run_agent`, which passes it through to `ensure_commits`.

**Branch on the field in `ensure_commits`:** Currently, when `has_unique_commits` returns false and there's a session ID, the function always resumes the session asking the agent to commit. Change this:

- **`OnNoCommits::Prompt`** (default): Keep the existing behavior — resume the session asking the agent to commit. No change.
- **`OnNoCommits::Sleep`**: Check whether the working tree is dirty (has modified/staged/untracked files). Use the existing `git_status` helper and `ls-files --others --exclude-standard` check from `worktree.rs`:
  - **Dirty**: The agent did work but forgot to commit. Resume the session asking it to commit (same as `Prompt`).
  - **Clean**: The agent genuinely found nothing. Return `CommitCheck::NoCommits` immediately — do **not** resume the session.

Add a new public function `is_working_tree_clean(worktree_path: &Path) -> Result<bool, WorktreeError>` in `worktree.rs` that combines the `git diff --quiet`, `git diff --cached --quiet`, and `ls-files --others --exclude-standard` checks from `land()` (lines 286-299) into a reusable function. This avoids duplicating the logic.

**After `CommitCheck::NoCommits` in `run_agent`:** When the agent's `on_no_commits` is `Sleep`, instead of just logging "skipping land", additionally call `wait_for_new_commits` before returning. This makes the worker sleep until there's new work to audit, breaking the infinite audit loop.

To do this, change `run_agent`'s return type or add a new variant. Simplest approach: add a `CommitCheck::Sleep` variant so the caller in `worker_loop` can call `wait_for_new_commits` (which needs `ctx.io`, `ctx.input`, etc. that `run_agent` already has access to via `ctx`). Actually, since `run_agent` already has `ctx`, it can call `wait_for_new_commits` directly inside the `NoCommits` arm when `on_no_commits == Sleep`.

### 3. Update the audit agent prompt (`.coven/agents/audit.md`)

Two changes:

**Add `on_no_commits: sleep` to frontmatter:**
```yaml
---
description: "Reviews codebase for quality issues and test gaps"
on_no_commits: sleep
---
```

**Add a "Quality over quantity" guideline to the prompt body**, appended to the existing Guidelines section:
```
- It's completely fine to find nothing — don't manufacture low-value issues just to have output. If everything looks good, simply finish without committing.
```

### 4. Tests

- **`src/agents.rs`**: Unit tests for `on_no_commits` parsing (both explicit `sleep` and default `prompt`).
- **`src/worktree.rs`**: Unit test for the new `is_working_tree_clean` function (clean tree returns true, modified/staged/untracked files return false).
- VCR tests aren't needed for this change — the behavior difference only triggers when an agent produces no commits, which is hard to reliably record. The unit tests plus the existing worker VCR tests cover the paths.
