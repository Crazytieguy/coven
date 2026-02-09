Issue: [P0] Create small issues to implement the design in design/orchestration.md
Status: draft

## Approach

Break the orchestration design into small, incremental issues that can each be implemented independently. Each issue should be a concrete, testable unit of work. The ordering below reflects dependency structure — later issues build on earlier ones.

### Key design principles (from the orchestration doc)

- Coven is a generic agent loop — workflow logic lives in prompt files, not code.
- Agent types are defined by `.coven/agents/*.md` files. The filename is the agent's identity.
- The issue file system (issues/, review/) is just markdown with YAML frontmatter. Coven doesn't need to understand issue files — agents interact with them via standard CLI tools. The dispatch prompt includes a bash command to list issues and their states.

### Proposed issues

#### Phase 1: Foundations (no existing behavior changes)

1. **Agent prompt file loading**
   Priority: P1
   Load agent definitions from `.coven/agents/*.md`. The filename (minus extension) is the agent name. Parse a small frontmatter format (description, required arguments). Provide a registry of available agent types. No behavioral integration yet — just the loading/parsing library.

2. **Git worktree primitives**
   Priority: P1
   Wrapper module for git worktree operations: spawn worktree (create branch + worktree at `~/worktrees/<project>/<branch>`, copy gitignored files) and land worktree (rebase onto main, ff-merge main, remove worktree + branch). Take inspiration from the example scripts in `design/spawn-worktree-example` and `design/land-worktree-example`. Thin wrappers around git CLI commands. Testable independently.

#### Phase 2: Single-worker loop

3. **`coven worker` subcommand (single cycle)**
   Priority: P1
   New subcommand that: creates a worktree, runs a single Claude session in it (hardcoded prompt for now — no dispatch yet), and exits. Reuses existing SessionRunner. This is the skeleton that everything else hangs on.

4. **Landing flow integration**
   Priority: P1
   After a worker's session ends with commits, run the landing flow (rebase + ff-merge from issue #2). Detect conflicts (but don't resolve them yet — just fail). Integrate into the worker subcommand.

5. **Dispatch agent integration**
   Priority: P1
   Replace the hardcoded prompt in `coven worker` with a dispatch cycle: run a short Claude session with the dispatch prompt (from `.coven/agents/dispatch.md`), parse its output (agent type + arguments or sleep), then run the chosen agent's prompt. Inject available agent types and their descriptions into the dispatch prompt. The dispatch prompt also includes a bash command for listing issues — coven doesn't parse issue files itself.

6. **Worker loop**
   Priority: P1
   After landing, loop back to dispatch instead of exiting. Implement sleep-until-new-commit (watch for changes on main via polling). Add break conditions (interrupt, no work available after N dispatches).

#### Phase 3: Multi-worker coordination

7. **Dispatch serialization**
   Priority: P1
   Ensure only one dispatch agent runs at a time across all `coven worker` processes. File lock or similar mechanism. Workers queue for the dispatch lock before running their dispatch cycle.

8. **Worker state sharing**
   Priority: P1
   Track what each worker is doing (agent type + arguments) in a shared location (e.g., `.coven/state/` with per-worker files). Inject other workers' status into each dispatch prompt so dispatch can make informed decisions.

9. **Conflict resolution flow**
   Priority: P2
   When rebase conflicts occur during landing, resume the Claude session with a prompt explaining the conflicts. After the session resolves them, retry the land. Currently (issue #4) conflicts just fail.

#### Phase 4: Polish

10. **Worker display in UI**
    Priority: P2
    Show agent type and arguments for each worker in the display. Make it obvious what each worker is doing.

11. **Default template scaffolding**
    Priority: P2
    Ship default `.coven/agents/` prompt files (dispatch, plan, implement, audit) and a default `workflow.md`. `coven init` or auto-creation on first `coven worker` run. Prompts should be heavily inspired by the current `workflow.md`, which works well in practice.

## Questions

### Should issues be created as files in `issues/` (the new format) or appended to `issues.md` (the current format)?

The orchestration design uses individual issue files with YAML frontmatter in `issues/` and `review/` directories. But the current workflow uses `issues.md` as a flat list. Creating the new issues as files would be using a system that doesn't exist yet. Creating them in `issues.md` keeps them in the current system but means they'll need to be migrated later.

Option A: Add to `issues.md` as one-liners (current system). Migrate when the new workflow is operational.
Option B: Create as files in `issues/` now, establishing the new convention early.

I'd lean toward Option A — keep using the current system until the new one is built.

Answer:

### Granularity — is this the right level of decomposition?

Each issue above is roughly a session or two of work. Some (like #3 and #5) could arguably be split further. Others (like #1 and #2) are small enough that splitting would add overhead without value. Is this level about right, or should issues be smaller/larger?

Answer:

### Should the standard template agent prompts be drafted now or deferred?

The orchestration design says agent types are defined by prompt files. We could draft the dispatch/plan/implement/audit prompts as part of this decomposition (informed by the current `workflow.md` which works well), or defer prompt writing to when each agent is integrated. Drafting early means the prompts can be reviewed alongside the design; deferring means they're written with more implementation context.

Answer:

## Review

