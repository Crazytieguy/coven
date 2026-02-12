---
priority: P1
state: approved
---

# Audit documentation for inconsistencies and redundancy

Audit documentation across the project to find inconsistencies and outdated documentation. Also find redundant documentation and unnecessary comments.

## Audit Summary

Audited all markdown files (README.md, CLAUDE.md, workflow.md, ideas.md), all code comments and doc comments across 32 source files, and cross-referenced documentation claims against actual behavior.

The templates/prompts audit issue covers template-specific findings (dispatch permissions, `claude_args` ordering, recording issues guidance). The code quality audit covers structural refactors. This plan covers only documentation-specific fixes: incorrect docs, stale files, redundant comments, and misleading doc comments.

## Plan

### 1. Fix incorrect default worktree base in README and CLI help

**Files:** `README.md:56`, `src/cli.rs:82`

Both say the default is `~/worktrees`, but `default_worktree_base()` in `src/main.rs:172-176` returns `~/.coven/worktrees`. Fix both to say `~/.coven/worktrees`.

### 2. Fix misleading `append_system_prompt` doc comment

**File:** `src/session/runner.rs:21`

Says "Append to system prompt (for ralph mode)." but the field is used by ralph, worker, and run commands. Change to "Append to system prompt."

### 3. Remove stale `ideas.md`

**File:** `ideas.md`

Early design brainstorming that predates the current implementation — configurable workflows, review agents, forking, etc. All of these have since been implemented or superseded. The file adds no value and could mislead.

Delete it.

### 4. Remove empty `best-practices-rejected.md`

**File:** `.claude/mats/best-practices-rejected.md`

Contains only a heading with no content. Delete it.

Review comment: No, this is used by the mats plugin. Please keep

### 5. Remove unnecessary inline comments in `worker.rs`

**File:** `src/commands/worker.rs`

Remove these comments that restate what the immediately following line does:

- Line 202: `// Load project config (entry agent name)` — next line is `let project_config: config::Config = ...`
- Line 311: `// Build system prompt: transition protocol + optional fork` — code is self-evident
- Line 319: `// Display agent header` (keep lines 318-328 as a visual section, just remove the comment)
- Line 330: `// Run the agent session` — before `run_phase_session(...)`
- Line 352: `// Parse transition from agent output (with retry on failure)` — before `parse_transition_with_retry(...)`

Keep the comment at line 211 (`// Sync worktree to latest main so the entry agent sees current state`) — it explains *why*, not just *what*. Also keep lines 306-307 (`// Merge per-agent claude_args...`) — the ordering semantics aren't obvious.

### 6. Remove trivial doc comments in `vcr.rs`

**File:** `src/vcr.rs:328, 333, 338`

Remove the doc comments on `is_live()`, `is_replay()`, and `is_record()`. The method names are perfectly self-documenting.

### 7. Standardize `cargo insta` guidance between README and CLAUDE.md

**Files:** `README.md:89`, `CLAUDE.md:18`

README says `cargo insta review` (interactive). CLAUDE.md says `cargo insta accept` (non-interactive, intended for agents). Both are valid commands but for different audiences. Change README to `cargo insta review` (already correct) and keep CLAUDE.md as `cargo insta accept` — but add a parenthetical to each clarifying the distinction: README gets "(interactive review)" and CLAUDE.md gets "(non-interactive)".

Review comment: I think we probably don't need testing documentation in the readme, just remove it (unless you think I'm wrong)

## Questions

- `worktree::land()` in `src/worktree.rs:269-337` duplicates `land.sh` exactly but is never called from production code (only tests). Should I file a separate issue to either remove the Rust function (relying solely on `land.sh`) or migrate agents to use the Rust function? This feels like a code quality issue more than documentation.

Answer: Remove the function, it's no longer used. Also remove the tests

- Should `ideas.md` be deleted outright, or would you prefer to keep it for historical reference?

Answer: delete
