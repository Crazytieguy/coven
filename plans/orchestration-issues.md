Issue: [P0] Create small issues to implement the design in design/orchestration.md
Status: draft

## Approach

Start with a small batch of foundational issues. Add more issues incrementally as these are completed and the design is validated in practice.

### Initial issues (this batch)

1. **VCR test infrastructure: git repo initialization**
   Priority: P1
   When the VCR recorder creates a temporary directory for a test case, it should also `git init` and create an initial commit. This makes VCR tests realistic — Claude sessions run inside git repos. Changes are in `record_vcr.rs` (recording setup) and potentially `vcr_test.rs` (replay setup if it creates temp dirs). Re-record all VCR fixtures afterward so they capture git-aware behavior, and update snapshots.

2. **Agent prompt file loading**
   Priority: P1
   New module for loading agent definitions from `.coven/agents/*.md`. The filename (minus extension) is the agent name. Parse YAML frontmatter (description, required arguments). Provide a registry of available agent types. No behavioral integration yet — just the loading/parsing module. Tested with regular unit tests (create temp dirs with agent files, verify parsing) — no VCR involvement.

3. **Git worktree primitives**
   Priority: P1
   New module wrapping git worktree operations as thin wrappers around git CLI commands:
   - **Spawn**: generate random branch name, `git worktree add -b`, copy gitignored files via rsync. Takes inspiration from `design/spawn-worktree-example`.
   - **Land**: rebase onto main, detect conflicts (return conflicting file list), ff-merge main to branch tip, remove worktree, delete branch. Takes inspiration from `design/land-worktree-example`. Coven's version should not remove the worktree after landing (worktree persists across dispatch cycles).
   Tested with regular unit tests using temporary git repos — no VCR involvement.

4. **Decompose next orchestration issues**
   Priority: P1
   Meta-issue: once issues 1–3 are complete, create the next batch of issues (worker subcommand, landing integration, dispatch, etc.). The right decomposition will be clearer after building the foundations.

### Deferred

- **Worktree support in VCR tests** (where to put worktrees relative to the test tmp dir) — deferred until we have a concrete test case that needs it.
- **Concurrent coven commands in tests** — deferred until needed for worker coordination testing.

## Questions

### Should worktree tests use real git operations or mocking?

The worktree primitives module wraps git CLI commands. Tests could either:
- **Real git**: create actual temp repos, run real git commands, verify results. Simple and high-fidelity but slower.
- **Mock/stub**: mock the `Command` calls. Faster but more brittle and less confidence.

The example scripts are straightforward CLI wrappers, so real git seems appropriate. But wanted to confirm.

Answer:

## Review

