---
priority: P1
state: approved with comments
---

# Audit template files and system prompts for redundancy and consistency

Audit all template files and system prompt additions for redundancy and consistency.

## Plan

Audited all 4 agent templates (`.coven/agents/*.md`), `workflow.md`, and the Rust code that generates/injects system prompts (`transition.rs`, `fork.rs`, `runner.rs`, `worker.rs`).

### Findings

**Consistency issues:**

1. **dispatch.md can't check worktree state**: The "Worktree State" section tells dispatch to detect commits ahead of main and dirty worktrees, but its only Bash permission is `head *`. It needs git commands to actually do this (e.g. `git log --oneline main..HEAD`, `git status`).

2. **`claude_args` ordering inconsistent across templates**: Each agent lists git commands in a different order — compare plan.md (`git add, git mv, git commit, git log, git diff, git status, git rebase`) vs land.md (`git log, git diff, git status, git add, git commit, git rebase`). Makes templates harder to compare at a glance.

**Redundancy (benign — no changes needed):**

3. `workflow.md` "Transition Protocol" section overlaps with the auto-injected system prompt from `transition.rs`. Agents in worker sessions see both. However, workflow.md also serves as human documentation and is referenced by interactive sessions via CLAUDE.md — keeping it is correct.

Review comment: no it's not, it should be removed

1. `workflow.md` "Default Agents" section repeats agent frontmatter descriptions. Serves as human-readable overview — keeping it is correct.

Review comment: no. Anything that is duplicated with the agent definitions or system prompt injections should be removed. Not meant for human overview. If anything suggests that workflow.md is meant for humans, that should be removed as well.

**Missing guidance:**

1. **Only implement.md has "Noticing Other Issues"**: Plan and land agents also explore code and could encounter unrelated bugs or tech debt, but lack guidance to record them.

Review comment: good catch, should be moved to workflow.md

### Changes

1. **dispatch.md: Add git permissions** — Add `Bash(git log:*),Bash(git diff:*),Bash(git status)` to `claude_args` so dispatch can fulfill its worktree state checking instructions.

2. **All agents: Standardize `claude_args` ordering** — Use consistent order across all templates: read-only commands (`git status`, `git log:*`, `git diff:*`), then mutating commands (`git add:*`, `git mv:*`, `git rm:*`, `git commit:*`), then workflow commands (`git rebase:*`, `bash .coven/land.sh`).

3. **plan.md and land.md: Add "Recording Issues" guidance** — Add a brief section similar to implement.md's "Noticing Other Issues", instructing these agents to create issue files for unrelated problems they encounter.
