# Orchestration Design — Brainstorming State

Status: early design, not ready for implementation.

Items are confirmed unless noted otherwise. Attribution is given for proposals that haven't been confirmed or didn't originate from the human.

## Core Mental Model

Concurrency-inspired architecture with git worktrees for isolation. Single writer per file, explicit ownership transfers.

### Principles

- **Worktree-per-agent.** Each worker gets its own git worktree. Management + human share the main worktree. Worktree spawned on agent start, removed on agent exit. Solves codebase consistency — every agent sees a stable snapshot.
- **Multiple execution agents.** No hard limit on concurrent execution. Management assigns work to minimize conflict risk, and can keep a worker idle if there's a blocking refactor.
- **Generic workers.** The human starts "workers", not role-specific agents. Coven (with management) dynamically assigns each worker a role/task per iteration — could be planning one iteration, execution the next, discovery after that. Minimum viable setup: one management terminal + one worker terminal.
- **Progressive adoption.** To get started, the human only needs to spin up management. Everything else is progressively introduced. Management recommends adding workers as needed.
- **Management recommends, human decides.** Management suggests adding/removing workers, flags issues that need attention, etc. Human is never under time pressure.
- **No file writable by more than one entity at the same time.** Ownership transfers are explicit. This is the core coordination rule.
- **Each Claude session should be as short as possible** — one atomic task. Stateless-by-default.
- **Per-agent-type instruction files.** Each agent type gets its own instructions file (e.g., management.md, planning.md, execution.md, discovery.md). Human can see and modify them, or tell agents to update them. Auto-loaded into context via the prompt.
- **Avoid over-engineering.** See how far CLI + editable files can go before building a desktop UI.

### Superseded Decisions

- ~~Single execution thread~~ → replaced by multiple execution agents with management-controlled concurrency.
- ~~Four independent roles as subcommands~~ → replaced by generic worker model. Only two subcommands: management and worker.

### Two Subcommands

No background processes. The human runs these manually in terminals:

- **Management** — at most one per project. Organizes issues, prioritizes, queues tasks for workers, writes end-of-session status with recommended actions.
- **Worker** — unlimited. Coven assigns it a role (planning, execution, discovery) and task each iteration based on management's queue.

The UI should make it obvious what role and task each worker is currently working on.

Agent/subcommand naming TBD (bakery theme floated).

### Interactive Design Sessions

Not a coven concept — just a usage pattern. When a plan is in review and the human wants help iterating on it, they can start a regular interactive claude session and point it at the issue file. Coven doesn't need to know about this. Management may suggest this in its status message when an issue is particularly vague.

### Thread Visibility

All agents are visible terminals the human can interact with. Both follow-up messages and steering messages work for all roles — follow-ups are more natural for management, steering is more natural for execution, but neither is restricted.

### Target Audience

Alignment researchers with academic backgrounds (MATS program). Interface must be friendly, simple, and steerable.

## Issue Lifecycle

### File States vs. Scheduling States

Some states live in the issue file's frontmatter. Others are scheduling concerns tracked by `schedule.toml` or coven's local state — they never appear in the issue file.

**File states** (in YAML frontmatter, reflected by filesystem location):
- `new` — issue exists in `.coven/issues/`, no plan yet
- `review` — plan written, file moved to `.coven/review/` for human attention
- `approved` / `changes-requested` — human has reviewed (file in `.coven/issues/`, moved back from `review/`)
- `done` / `needs-replan` — execution finished

**Scheduling states** (not in frontmatter, implicit from `schedule.toml` and coven):
- "planning-ready" — management has queued a planning task for this issue in `schedule.toml`
- "being planned" — a worker is currently working on it
- "queued for execution" — management has queued an execution task in `schedule.toml`
- "being executed" — a worker is currently working on it

Open question: status names need reconsideration. "approved" makes sense for a plan but is awkward for an issue that skipped planning. (Noted for next session.)

### State Transitions

```
State              Who                What happens
─────              ───                ────────────
new                management         triages, sets priority, queues planning task
                                      for a specific worker in schedule.toml
(being planned)    worker             writes plan section, sets state to review,
                                      moves file to .coven/review/ (open question:
                                      worker or coven does the move? leaning worker)
review             human              reads plan, either approves or leaves comments,
                                      sets status explicitly when done
changes-requested  management         reads comments, decides: revise plan, split
                                      issue, drop, etc. queues accordingly
approved           management         decides execution order, queues execution task
                                      for a specific worker in schedule.toml
(being executed)   worker             writes code, sets state to done or needs-replan
needs-replan       management         reads notes, queues re-planning
done               —                  cleaned up (mechanism TBD)
```

### Details

- **Skip path**: human tells management (usually when creating the issue) that it doesn't need a plan. Management sets the issue state directly to `approved`. Only the human has this affordance, not management.
- **Vague issues**: the planner should leave questions in the plan rather than guess. Can recommend the human iterate interactively. How detailed a plan vs. how many questions is a planning concern, not a management concern.
- **Human review**: two actions only — approve or leave comments. No special statuses beyond that. Management reads comments and decides what to do (revise, split, drop, etc.). Keeps the human's mental model simple.
- **Explicit status trigger**: human must explicitly set the status field when done reviewing. Management ignores the plan until then — no time pressure.
- **Plans always route through management** after review or execution. Sometimes inefficient, but always safe.
- **Human generally doesn't create issue files directly** — messages management instead.
- **Management never writes plan content** — only triages, queues, and routes. Sometimes writes initial issue content, but not the plan section.

## File Structure

```
.coven/
  schedule.toml              # worker queues (management is sole writer)
  agents/
    management.md            # per-agent-type instructions
    planning.md
    execution.md
    discovery.md
  issues/                    # issues not currently in review
    fix-scroll-bug.md
    add-dark-mode.md
  review/                    # plans waiting for human review
    refactor-renderer.md
```

### Issue File Format

Markdown with YAML frontmatter. Filename is the issue ID (kebab-case).

A new issue:
```markdown
---
priority: P1
state: new
---

# Fix scroll bug

Scroll position resets on resize.
```

An issue in review (lives in `.coven/review/`):
```markdown
---
priority: P1
state: review
---

# Fix scroll bug

Scroll position resets on resize.

## Plan

Refactor the scroll handler to preserve position across resize events.

## Questions

### Should we debounce?

Resize events fire rapidly...

**Answer:**
```

### schedule.toml

Per-worker queues plus one backup queue (for when the human adds a new worker). Management can rebalance queues. Each queue item has an agent type and issue ID (if relevant). Management is the sole writer.

Format likely needs changes — below is illustrative:

```toml
[[backup]]
agent = "planning"
issue = "add-dark-mode"

[[workers.worker-1]]
agent = "execution"
issue = "fix-scroll-bug"

[[workers.worker-1]]
agent = "planning"
issue = "refactor-renderer"

[[workers.worker-2]]
agent = "discovery"
```

### Superseded

- ~~`board.toml` with issues, priorities, queue, agent status~~ → issues in their own files, schedule.toml is just worker queues.
- ~~Separate plan files in `plans/`~~ → plan is a section in the issue file.
- ~~`issues.md` as a flat list~~ → one file per issue in `.coven/issues/`.

## Worktree Model

- **Worktree per worker.** Spawned on agent start (similar to `spawn-worktree` script), removed on agent exit. Persistent across sessions within the agent's lifetime.
- **All local.** No pushing to remote by default.
- **Management + human share the main worktree.**
- **Main worktree only updated between management sessions.** During a management session, the main worktree sees a stable snapshot. Worker landings don't update the main worktree until the management session ends.

### Landing Flow

When a worker finishes a task, coven handles the git operations on the worker's worktree:

1. Worker's claude session ends (code/plan committed by the agent on the worktree branch)
2. Coven rebases the worktree branch onto main (picking up any changes other workers have landed)
3. If rebase conflicts → coven starts a conflict resolution claude session on the worker's worktree. Worker resolves conflicts and commits.
4. Once clean → coven fast-forward merges main to the worktree branch tip. This just moves main's branch pointer forward — no merge commit, since the branch is already rebased on top of main.
5. Coven picks up the worker's next task from schedule.toml

Workers rebase and continue with their next task even during management sessions — but the fast-forward merge to main (step 4) is deferred until between management sessions. Workers don't block on management.

Between management sessions, coven also updates the main worktree to match the (now advanced) main branch. If the main worktree has uncommitted changes from the human that conflict with landed commits, the update may need to abort and retry. (Details TBD — see notes for next session about management on a worktree.)

### Coordination

- **schedule.toml**: management is the sole writer. Workers/coven only read it. Coven tracks each worker's queue progress locally (in-memory).
- **Management cleans up** completed items from schedule.toml on its next session.
- **Workers that finish during a management session** rebase onto main and continue with their next queued task from the committed schedule.toml. Their commits stay unlanded until the management session ends.
- **Two workers with conflicting commits**: first to land succeeds, second hits conflict during rebase on its own worktree, resolves via conflict resolution session, then lands.
- **Discovery naming conflicts** (two workers create issue files with the same name): rare, worker resolves.

## State Changes by Entity

**Human (main worktree):**
- Messages management to create issues, set priorities, flag issues as not needing plans, etc.
- Reviews plans in `.coven/review/` — edits content, sets status to `approved` or `changes-requested`
- Can start interactive claude sessions at any time to help iterate on plans or brainstorm

**Management agent (main worktree):**
- Triages issues: sets priority, queues planning tasks for specific workers
- Pushes tasks onto worker queues in `schedule.toml`
- Routes post-review issues (reads human comments, queues revision or execution accordingly)
- Routes `needs-replan` back to planning
- Creates initial issue content
- Writes end-of-session status with recommended actions
- Never writes plan content

**Worker (own worktree):**
- Planning: writes plan section in issue file, sets state to `review`, moves file to `.coven/review/`, commits
- Execution: writes code, sets state to `done` or `needs-replan` (with notes), commits
- Discovery: creates new issue files (state: `new`), commits
- Conflict resolution: resolves merge conflicts on own worktree, commits

**Coven (process-level):**
- Manages worktree lifecycle (spawn on agent start, remove on exit)
- Rebases worker worktree onto main between tasks
- Fast-forward merges worker commits to main branch between management sessions
- Updates main worktree between management sessions
- Reads schedule.toml, tracks worker progress locally, picks next task
- Starts claude sessions with appropriate prompt and agent-type instructions
- State corruption recovery: if management leaves schedule.toml unparsable, coven should auto-follow-up to fix it (carried over, not re-discussed)

## Open Questions

### Discovery
- How are discovery threads configured per-project?
- What kinds of discovery are supported? (QA, code review, test proposals, feature proposals — all mentioned but not specified.)
- Discovery deduplication: management deduplicates issue suggestions. (Proposed, not confirmed.)

### Management
- How does management wake up? Events, polling, or both?
- Intelligent cap on pending plans — too many queued plans go stale as the codebase changes.
- End-of-session status format and content.

### UI
- How does the human distinguish which terminal they're typing in?
- How does the human see what management has done without scrolling through terminal history?

### Lifecycle
- What "cleaned up" means for completed issues.
- Issue status names need reconsideration — "approved" is awkward for issues that skip planning.

## Notes for Next Session

- **Should management be on its own worktree?** That way it never sees dirty working tree state as the human is editing. Need to consider what this means for syncing between management and the human.
- **Should the worker pop from the schedule?** Currently coven tracks progress locally, but this means schedule.toml is "wrong" (shows completed items as still queued). Need to think about how this interacts with management rebalancing queues.
