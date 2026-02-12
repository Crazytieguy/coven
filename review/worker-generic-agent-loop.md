---
priority: P0
state: review
---

# Redesign worker as a generic agent loop

## Motivation

The current worker hardcodes a rigid dispatch→agent→land pipeline with special-cased dispatch parsing, automatic rebase-to-main after every agent, and a single exclusive lock. This limits workflows: the implement agent must complete an entire issue in one session, there's no way to chain agents (e.g. implement → verify → land), and the dispatch agent has custom XML parsing logic that nothing else uses.

## Design

### Generic agent loop

The worker loop becomes:

```
1. Create worktree, sync to main
2. Run entry agent (configurable, default: "dispatch")
3. Parse transition from agent output
4. Handle transition:
   a. next: <agent> → run agent, goto 3
   b. sleep → wait for new commits on main, goto 2
```

Every agent — including dispatch — outputs a `<next>` tag at the end of its session to declare what should happen next. Coven parses this universally; no agent gets special treatment in code.

There is no special `land` transition — landing (rebase to main) is handled by agents via git commands, not by coven.

### Transition protocol

All agents end their session with a `<next>` tag containing YAML:

```yaml
# Hand off to another agent:
<next>
agent: implement
args:
  issue: issues/fix-scroll-bug.md
</next>

# Sleep (wait for new commits on main):
<next>
sleep: true
</next>
```

Coven parses the last assistant message for a `<next>` tag. If missing or malformed, coven resumes the session with a reminder of the expected syntax.

### Entry agent configuration

`.coven/config.toml` specifies which agent runs first:

```toml
entry_agent = "dispatch"
```

After waking from `sleep`, the worker always returns to the entry agent.

### Context injection: agent catalog and worker states

- **Agent catalog**: Auto-appended to every agent's rendered prompt. Every agent needs to know the `<next>` transition protocol and available agents. Not a template variable — appended automatically by coven after rendering.
- **Worker status**: Remains a template arg (`worker_status`). Agents opt in by declaring it in their frontmatter. Before rendering, the worker generates the status string and injects it into any agent that declares this arg.

### What coven handles

- Worktree creation and cleanup
- Syncing worktree to main before entry agent
- Transition parsing and routing
- Dispatch lock for the entry agent (temporary — will be replaced by per-agent semaphores in a follow-up issue)
- Sleep/wake (watching main refs)

### What agents handle

- All domain logic (reading issues, writing plans, implementing, etc.)
- All git operations (branching, committing, rebasing, merging to main)
- Deciding what happens next (via `<next>` tag)

## Plan

### Step 1: Rename `dispatch.rs` → `transition.rs`

Rename the module and update types:
- `DispatchDecision` → `Transition`
- `DispatchDecision::RunAgent { agent, args }` → `Transition::Next { agent, args }`
- `DispatchDecision::Sleep` → `Transition::Sleep`
- `parse_decision()` → `parse_transition()`
- Tag name: `<dispatch>` → `<next>`
- Update `format_agent_catalog()` to:
  - Use `<next>` tag in examples instead of `<dispatch>`
  - Include ALL agents (not just non-dispatch), since any agent can transition to any other
  - Rename section header to "Transition Protocol"
- Update `src/lib.rs`: `pub mod dispatch` → `pub mod transition`
- Update `src/commands/worker.rs` imports
- Rename `dispatch.rs` unit tests accordingly

### Step 2: Add `src/config.rs`

Minimal config module for `.coven/config.toml`:
```rust
pub struct Config {
    pub entry_agent: String, // default: "dispatch"
}
```
- `cargo add toml`
- `load(worktree_path)` → parse `.coven/config.toml`, fall back to defaults if missing
- Register in `src/lib.rs`

### Step 3: Rewrite `worker_loop` as generic agent loop

Replace the current dispatch→agent→land flow with:

```
outer loop:
  cleanup_if_dirty(worktree)    // abort rebase, git clean -fd, git checkout -- .
  sync_to_main(worktree)

  agent_name = entry_agent (from config)
  agent_args = {}
  is_entry = true

  inner loop (agent chain):
    if is_entry:
      acquire dispatch lock
      read worker states for injection

    load agent defs from .coven/agents/
    find agent_def by agent_name (error if unknown)
    build args: merge transition args + auto-inject worker_status if declared
    render agent prompt via Handlebars
    append agent catalog to rendered prompt

    run session (lock held during entry agent session)
    update worker state with current agent/args
    drop lock (if held)

    parse <next> from session output (with retry on failure, see step 5)

    match transition:
      Next { agent, args } → update agent_name/args, is_entry = false, continue
      Sleep → clear worker state, wait_for_new_commits, break to outer loop
```

**Key behavior decisions:**
- Sync to main happens only before the entry agent (outer loop), not between chained agents. Chained agents may have uncommitted work or work that hasn't been landed yet.
- Cleanup before entry: if the worktree has an in-progress rebase (from crash) or uncommitted changes, clean up before syncing. This is a safety net — agents should handle their own cleanup, but crashes happen.
- Dispatch lock is kept temporarily for the entry agent only (same as current behavior). The concurrency issue will replace this with per-agent semaphores.

### Step 4: Remove landing code from worker

Delete all landing-related code from `src/commands/worker.rs`:
- **Functions to delete**: `run_agent()` (the current version with landing), `ensure_commits()`, `ensure_clean()`, `land_or_resolve()`, `try_land()`, `resolve_conflict()`, `handle_ff_retry()`, `handle_land_error()`, `handle_conflict()`, `abort_and_reset()`
- **Enums to delete**: `CommitCheck`, `CleanCheck`, `LandAttempt`, `ResolveOutcome`
- **Constants to delete**: `MAX_CLEANUP_ATTEMPTS`, `MAX_LAND_ATTEMPTS`
- **VCR wrappers to delete**: `vcr_has_unique_commits`, `vcr_dirty_state`, `vcr_is_rebase_in_progress`, `vcr_abort_rebase`

**Keep**: `run_phase_session()`, `wait_for_new_commits()`, `wait_for_enter_or_exit()`, `warn_clean()` (used in cleanup), `vcr_update_worker_state()`, `vcr_main_head_sha()`, `vcr_resolve_ref_paths()`, all ref watcher code.

Replace with a simple `run_agent_session()` that runs the session and returns the result text (no landing, no commit checking).

### Step 5: Generalize transition parsing retry

Extract the current dispatch retry logic (worker.rs lines 241-283) into a generic function usable for any agent:
- After any session, try `parse_transition()` on the result text
- If `<next>` tag is missing or malformed, resume the session with a corrective prompt explaining the `<next>` YAML format
- Retry once; if still fails, error out
- The corrective prompt includes agent-catalog-style examples of valid `<next>` output

### Step 6: Auto-append agent catalog

After rendering any agent's prompt template (via `AgentDef::render()`), append the agent catalog text. This is done in the worker loop, not in the render method itself (render stays pure).

The appended catalog includes:
- List of all agents with descriptions and arguments
- `<next>` YAML syntax with examples for each agent
- `sleep: true` syntax

Remove `agent_catalog` from the dispatch agent's frontmatter `args` (it's no longer a template variable).

### Step 7: Auto-inject `worker_status` for agents that declare it

Before rendering an agent, check if it has a `worker_status` arg in its frontmatter. If so:
- Read all worker states (via `worker_state::read_all()`)
- Format status (via `worker_state::format_status()`)
- Add to the args map

This replaces the current hardcoded injection in `run_dispatch()`. The dispatch agent keeps `worker_status` as a required arg in its frontmatter; other agents can add it if they want.

### Step 8: Update agent templates

**`.coven/agents/dispatch.md`**:
- Remove `agent_catalog` from args (auto-appended now)
- Keep `worker_status` as required arg
- Change output instructions from `<dispatch>` to `<next>` format
- Add routing for "worktree ahead of main" → `next: land`

**`.coven/agents/plan.md`**:
- Add transition instruction: after completing, output `<next>agent: dispatch</next>`
- Plan agent still lands its own commits directly (rebase to main) for lightweight changes

**`.coven/agents/implement.md`**:
- Add transition instructions:
  - On success: `<next>agent: land</next>`
  - On needs-replan: `<next>agent: dispatch</next>`

**`.coven/agents/land.md`** (new):
- Description: "Audits changes and lands them on main"
- No required args (operates on current worktree state)
- Reviews the diff between branch and main
- Does final cleanup/code review if needed
- Runs `git rebase <main>` then `git checkout main && git merge --ff-only <branch>` (or equivalent)
- Transitions:
  - Success: `<next>agent: dispatch</next>`
  - Needs more work: `<next>agent: land</next>` (retry)
  - Fundamental problems: mark issue as needs-replan, `<next>agent: dispatch</next>`

### Step 9: Update `init.rs`

- Add `land.md` to `AGENT_TEMPLATES` array (new `include_str!`)
- All embedded templates pick up the changes from step 8 automatically (they're `include_str!` from `.coven/agents/`)

### Step 10: Update documentation

**`README.md`**: Change "dispatch → agent → land loop" description to reflect the generic agent loop.

**`.coven/workflow.md`**: Update to describe:
- The `<next>` transition protocol
- The agent chain model
- The land agent's role

### Step 11: Re-record VCR tests

Re-record orchestration VCR fixtures that test the worker flow. Affected tests:
- `worker_basic` — the core dispatch→agent flow changes to dispatch→agent→(transition-based routing)
- `concurrent_workers` — multiple workers now use the generic loop
- `landing_conflict` — landing is now agent-driven, not coven-driven (this test may need significant restructuring)
- `needs_replan` — transition flow changes
- `plan_ambiguous_issue` — plan agent now outputs `<next>`
- `priority_dispatch` — dispatch output format changes from `<dispatch>` to `<next>`

Non-orchestration tests (session, rendering, fork, ralph, subagent) should be unaffected.

Update test `.toml` fixtures to use `<next>` tag format and include the land agent template.

After re-recording: `cargo insta review` to verify snapshot diffs look correct.

## Questions

1. **Land agent naming**: The issue mentions `land`, `finalize`, `review-and-land`, `ship`. I've used `land` throughout the plan. Do you prefer a different name?

2. **Agent catalog auto-append vs template variable**: The plan auto-appends the catalog to every agent prompt (agents can't control placement). Alternative: make it a template variable `{{agent_catalog}}` that agents include explicitly, with a build-time warning if an agent doesn't reference it. Which do you prefer?

3. **Cleanup aggressiveness before entry agent**: If the worktree is dirty on re-entry (e.g., after crash), the plan discards uncommitted changes and aborts any in-progress rebase. Committed-but-unlanded work is preserved (dispatch can detect "ahead of main" and route to land). Is this acceptable, or do you want more cautious recovery?
