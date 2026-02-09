# Orchestration Design — Brainstorming State

Status: early design, not ready for implementation.

Items are confirmed unless noted otherwise. Attribution is given for proposals that haven't been confirmed or didn't originate from the human.

## Core Mental Model

Coven is a generic agent loop. It manages worktrees, runs Claude sessions, and lands commits. The workflow logic lives in customizable prompt files, not in coven's code.

### Principles

- **Worktree-per-worker.** Each worker gets its own git worktree. The human works on the main worktree. Worktree spawned on worker start, removed on worker exit.
- **Multiple workers.** No hard limit on concurrent workers.
- **Generic workers.** The human starts "workers", not role-specific agents. Coven dynamically assigns each worker an agent via the dispatch agent.
- **Workflow is data, not code.** Agent types are defined by prompt files. The standard template includes dispatch, plan, implement, and audit. Users can modify prompts, add new agent types, or change the workflow entirely. Coven doesn't encode workflow logic beyond running the agent loop.
- **Each Claude session should be as short as possible** — one atomic task. Stateless-by-default.
- **Per-agent-type instruction files.** Each agent type gets its own prompt file. Human can see and modify them. Auto-loaded into context by coven.
- **Avoid over-engineering.** See how far CLI + editable files can go before building a desktop UI.

### Superseded Decisions

- ~~Single execution thread~~ → replaced by multiple workers.
- ~~Four independent roles as subcommands~~ → replaced by generic worker model.
- ~~Management agent + schedule.toml~~ → replaced by dispatch agent. No centralized scheduler — each worker dispatches itself, serialized by coven.
- ~~Management + human share the main worktree~~ → only the human uses the main worktree.

## Agent Types (Standard Template)

Four agent types ship with the default template. Each is defined by a prompt file in `.coven/agents/`. Each agent has a description field that tells the dispatch agent what it's for, and declares what arguments it requires.

- **Dispatch** — Chooses what a worker should do next. Outputs an agent type + arguments (e.g. `plan issues/fix-scroll-bug.md`), or sleeps. Also outputs brief reasoning/status visible to the human. Only one dispatch agent runs at a time (serialized by coven).
- **Plan** — Takes an issue file as argument. Writes a plan section in the issue, sets state to `review`, moves file to `review/`, commits. May create new issue files (splitting).
- **Implement** — Takes an issue file as argument. Writes code, sets state to `done` or `needs-replan` (with notes), commits. May create new issue files for things it notices along the way.
- **Audit** — Takes no arguments. Routine maintenance: code review, test gaps, quality issues. Creates new issue files for findings, commits.

Coven injects the available agent types, their descriptions, their required arguments, and the dispatch output syntax into the dispatch prompt. This way the dispatch prompt doesn't need to be updated when agents are added or modified.

The standard template also includes a `workflow.md` at the project root explaining the issue system. CLAUDE.md links to it so interactive Claude sessions understand the workflow too. The agent prompt files should be heavily inspired by the current `workflow.md`, which works well in practice.

### Dispatch

The dispatch agent's prompt includes:
- A recommended command to run in order to view all issues and their states
- What each other worker is currently doing (agent type + arguments), injected by coven
- The available agent types, their descriptions, arguments, and dispatch syntax, injected by coven

The dispatch agent understands the full workflow and makes intelligent decisions — priorities, codebase locality, throttling plans when the review queue is long, sleeping when outstanding work blocks everything. These concerns live in the prompt, not in coven's code.

### Interactive Design Sessions

Not a coven concept — just a usage pattern. When a plan is in review and the human wants help iterating on it, they can start a regular interactive claude session and point it at the issue file.

### Thread Visibility

All workers are visible terminals the human can interact with. Both follow-up messages and steering messages work for all agent types. The UI should make it obvious what agent and arguments each worker is currently working on.

### Target Audience

Alignment researchers with academic backgrounds (MATS program). Interface must be friendly, simple, and steerable.

## Worker Lifecycle

1. Human starts a worker (`coven worker`)
2. Coven creates a worktree, runs the **dispatch** agent (acquiring the dispatch lock first)
3. Dispatch outputs an agent type + arguments (or sleep)
4. Coven releases the dispatch lock
5. If sleep → worker waits for a new commit on main, then back to step 2
6. Coven starts the chosen agent session on the worktree
7. Agent does work, commits
8. Coven lands (rebase onto main + ff-merge main to worktree tip)
9. If land fails (rebase conflict) → coven resumes the session for conflict resolution, retry land
10. Back to step 2

## Issue Lifecycle

### File States

States live in the issue file's YAML frontmatter and are reflected by filesystem location:

- `new` — issue exists in `issues/`, no plan yet
- `review` — plan written, file moved to `review/` for human attention
- `approved` / `changes-requested` — human has reviewed (file in `issues/`, moved back from `review/`)
- `needs-replan` — implementation failed, needs revised plan

Scheduling state (which worker is working on what) is tracked by coven, not in files.

Open question: status names need reconsideration. "approved" is awkward as a general term for "ready to implement."

### State Transitions

```
State              Who                What happens
─────              ───                ────────────
new                plan agent         writes plan section, sets state to review,
                                      moves file to review/, commits.
                                      may split into multiple issues.
review             human              reads plan, approves or leaves comments,
                                      sets status, moves file back to issues/,
                                      commits
changes-requested  plan agent         revises plan (may split the issue)
approved           implement agent    writes code, deletes the issue file on success,
                                      or sets state to needs-replan on failure
needs-replan       plan agent         revises plan based on implementation notes
```

### Details

- **Skip path**: human creates an issue and sets state directly to `approved`. Dispatch routes to implement.
- **Vague issues**: the plan agent should leave questions in the plan rather than guess. Can recommend the human iterate interactively.
- **Human review**: two actions only — approve or leave comments. No time pressure.
- **Creating issues**: the human writes issue files and commits on the main worktree. Since the workflow is documented in CLAUDE.md, the human can also ask an interactive Claude session to create issues.
- **Agents can create issues**: both plan (splitting) and implement (things noticed along the way) can create new issue files.

## File Structure

```
workflow.md                  # explains the issue system (linked from CLAUDE.md)
issues/                      # issues not currently in review
  fix-scroll-bug.md
  add-dark-mode.md
review/                      # plans waiting for human review
  refactor-renderer.md
.coven/
  agents/
    dispatch.md              # dispatch agent prompt
    plan.md                  # plan agent prompt
    implement.md             # implement agent prompt
    audit.md                 # audit agent prompt
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

An issue in review (lives in `review/`):
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

## Worktree Model

- **Worktree per worker.** Spawned on worker start, removed on worker exit. Persistent across sessions within the worker's lifetime.
- **All local.** No pushing to remote by default.
- **Only the human uses the main worktree.**

### Landing Flow

When a worker finishes a task, coven handles the git operations (same as `land-worktree` script, but without removing the worktree):

1. Worker's Claude session ends (work committed on the worktree branch)
2. Coven rebases the worktree branch onto main
3. If rebase conflicts → coven resumes the Claude session for conflict resolution, then retries from step 2
4. Once clean → coven fast-forward merges main to the worktree branch tip (moves main's pointer forward)
5. Worker proceeds to next dispatch

### Coordination

- **Dispatch serialization**: coven ensures only one dispatch agent runs at a time. Eliminates races where two workers try to pick the same issue.
- **Worker state**: coven tracks what each worker is doing (agent type + arguments) and injects this into dispatch prompts. Since each worker is its own `coven worker` process, this requires inter-process coordination.
- **Two workers with conflicting commits**: first to land succeeds, second hits conflict during rebase on its own worktree. Coven resumes the session for conflict resolution, then lands.
- **Workers wake up on new commits to main** when sleeping (waiting for work).

## Open Questions

- Issue status names need reconsideration — "approved" is awkward as a general term.
- Exact mechanism for dispatch serialization and worker state sharing across `coven worker` processes. Options surfaced during implementation:
  - **File lock** for dispatch serialization (e.g. `.coven/dispatch.lock` using `flock`). Simple, no daemon needed.
  - **State directory** for worker status (e.g. `.coven/workers/<pid>.json` with agent type + args). Each worker writes its own file, dispatch reads all of them. Stale files cleaned up by PID liveness check.
  - **Unix socket** — a `coven daemon` process manages state and serialization. More complex but cleaner API.
  - Current implementation supports single-worker only (no serialization, hardcoded "no other workers active" status).
- "Audit" naming — should be a verb, and the exact scope of what it covers needs refinement.
- **Permission mode for worker sessions** (raised during implementation): agents need tool access (bash, file edit) to do real work. The default `acceptEdits` only allows file edits. Should `coven worker` default to a more permissive mode like `bypassPermissions`, or require the user to pass `--permission-mode` via extra args?
- **Conflict resolution via session resume** (raised during implementation): when `land` hits a rebase conflict, the design says coven resumes the Claude session for conflict resolution. Current implementation aborts the rebase instead and lets dispatch re-evaluate. Implementing the resume path requires tracking the agent session ID across the land phase. Worth doing, or is abort-and-redispatch good enough?
- **Workflow documentation** (raised during init implementation): the design says the standard template includes a `workflow.md` explaining the issue system, linked from CLAUDE.md. Currently `workflow.md` describes the ralph-mode workflow. Should `coven init` create/replace `workflow.md` with issue-system docs? Options: (a) init creates a separate file like `.coven/workflow.md` that CLAUDE.md links to, (b) init replaces the project-root `workflow.md`, (c) init doesn't touch workflow docs — the human writes them. Related: should init also update CLAUDE.md to reference the agent workflow?
