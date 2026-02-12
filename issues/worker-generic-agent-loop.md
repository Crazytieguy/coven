---
priority: P0
state: new
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
   a. next: <agent> → acquire semaphore, run agent, goto 3
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

### Per-agent concurrency control

Agent frontmatter gains a `max_concurrency` field:

```yaml
---
description: "Route work to agents based on issue state"
max_concurrency: 1
args: ...
---
```

Before running an agent, the worker acquires a semaphore permit for that agent type. This replaces the current exclusive dispatch lock with a generic mechanism. Implementation: counted file locks in `<git-common-dir>/coven/semaphores/`.

Default if unspecified: unlimited (no concurrency restriction).

### Context injection: agent catalog and worker states

**Open question for planning:** Currently, agent_catalog and worker_status are injected only into the dispatch agent as special args. In the new system, all agents need the agent catalog (so they know what transitions are available). It's unclear whether all agents need worker states. Options to explore:
- Automatically inject agent_catalog into all agents (not declared in frontmatter)
- Keep it as an explicit arg that agents opt into
- Some hybrid approach

### What coven handles

- Worktree creation and cleanup
- Syncing worktree to main before entry agent
- Transition parsing and routing
- Concurrency semaphore management
- Sleep/wake (watching main refs)

### What agents handle

- All domain logic (reading issues, writing plans, implementing, etc.)
- All git operations (branching, committing, rebasing, merging to main)
- Deciding what happens next (via `<next>` tag)

## Default template workflow

### Dispatch agent (max_concurrency: 1)

Reads issue queue and worker status. Routes work:
- Approved issue → `next: implement`
- New/changes-requested/needs-replan issue → `next: plan`
- Worktree ahead of main (unmerged work) → `next: land` (see below)
- Nothing to do → `sleep`

### Plan agent

Reads issue, explores codebase, writes plan, moves issue to `review/`. Lands its own plan commit directly (rebases to main). Transitions to `next: dispatch`.

### Implement agent

Implements the approved plan. On success → `next: land`. If needs another pass → `next: implement`. If stuck → marks issue as `needs-replan`, transitions to `next: dispatch`.

### Land agent

**Naming TBD** — this agent also does code review/audit, not just landing. Possible names: `land`, `finalize`, `review-and-land`, `ship`.

Runs after implement (or when dispatch detects the worktree is ahead of main). Audits the branch for final cleanup, does a code review pass, then rebases to main and merges. Possible transitions:
- Success → `next: dispatch` (pick up more work)
- Needs more cleanup → `next: land` (another pass)
- Fundamental problems → marks issue as `needs-replan`, transitions to `next: dispatch`

## Key changes from current system

1. **No special dispatch logic in code** — dispatch is just an agent with max_concurrency: 1
2. **Universal transition protocol** — all agents use `<next>`, replacing dispatch-only `<dispatch>` parsing
3. **Agent chaining** — implement can call itself, hand off to land, etc.
4. **Semaphore replaces lock** — generic, per-agent-type, counted
5. **Configurable entry point** — not hardcoded to "dispatch"
6. **Agents own git operations** — coven no longer handles landing; the land agent does
