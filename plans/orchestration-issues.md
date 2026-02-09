Issue: [P0] Create small issues to implement the design in design/orchestration.md
Status: draft

## Approach

Start with a small batch of foundational issues. Add more issues incrementally as these are completed and the design is validated in practice. Issues will be created as files in `issues/` following the new format (YAML frontmatter, markdown body).

### Initial issues (this batch)

1. **Orchestration testing infrastructure**
   Priority: P1
   Set up the testing harness for orchestration features. This includes: helpers to initialize temporary git repos with commits (for worktree tests), helpers to create `.coven/agents/*.md` files in test fixtures, and any shared test utilities needed by subsequent issues. This is foundational — every orchestration issue depends on it.

2. **Agent prompt file loading**
   Priority: P1
   Load agent definitions from `.coven/agents/*.md`. The filename (minus extension) is the agent name. Parse YAML frontmatter (description, required arguments). Provide a registry of available agent types. No behavioral integration yet — just the loading/parsing module. Tests use the infrastructure from issue 1 to set up agent files and verify loading.

3. **Git worktree primitives**
   Priority: P1
   Wrapper module for git worktree operations: spawn worktree (create branch + worktree, copy gitignored files) and land worktree (rebase onto main, ff-merge main, remove worktree + branch). Take inspiration from the example scripts in `design/spawn-worktree-example` and `design/land-worktree-example`. Thin wrappers around git CLI commands. Tests use temporary git repos from issue 1.

4. **Decompose next orchestration issues**
   Priority: P1
   Meta-issue: once issues 1–3 are complete, create the next batch of issues (worker subcommand, landing integration, dispatch, etc.). The right decomposition will be clearer after building the foundations.

## Questions

None — previous questions were answered:
- Use the new issue file format (`issues/` with YAML frontmatter)
- Granularity is decent
- Agent prompts should each be their own issue

## Review

