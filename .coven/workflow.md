# Orchestration Workflow

This project uses [coven](https://github.com/Crazytieguy/coven) for orchestrated development. Multiple workers run simultaneously, each picking up tasks from the issue queue.

## Agent Loop

Workers run a generic agent loop:

1. Sync worktree to main
2. Run entry agent (default: `dispatch`, configurable in `.coven/config.toml`)
3. Parse `<next>` transition from agent output
4. Handle transition:
   - `agent: <name>` → run that agent, goto 3
   - `sleep: true` → wait for new commits on main, goto 1

Every agent outputs a `<next>` tag at the end of its session to declare what should happen next. Coven parses this universally and injects the transition protocol into every agent's system prompt.

## Issue Files

Issues are markdown files with YAML frontmatter in `issues/` or `review/`.

```yaml
---
priority: P1
state: new
---

# Fix scroll bug

Scroll position resets on window resize.
```

### Priorities

- `P0` — Critical, blocks other work
- `P1` — Normal priority (default)
- `P2` — Nice to have

### States

| State | Directory | Meaning |
|-------|-----------|---------|
| `new` | `issues/` | No plan yet — plan agent will pick it up |
| `review` | `review/` | Plan written, waiting for human review |
| `approved` | `issues/` | Human approved the plan, ready to implement |
| `changes-requested` | `issues/` | Human left feedback on the plan |
| `needs-replan` | `issues/` | Implementation failed, plan needs revision |

### Lifecycle

```
new → review              Plan agent writes plan, moves file to review/
review → approved         Human approves, moves file back to issues/
review → changes-requested  Human requests changes, moves file back to issues/
changes-requested → review  Plan agent revises, moves file to review/
approved → (deleted)      Implement agent succeeds, deletes the issue
approved → needs-replan   Implement agent fails, adds notes
needs-replan → review     Plan agent revises based on failure notes
```

## Creating Issues

Create a markdown file in `issues/` with the format above. Minimum fields: `state` and `priority` in frontmatter, plus a title and description. Commit the file.

**Skip path**: To skip planning and go straight to implementation, set `state: approved`.

## Recording Issues

Always record issues you encounter that are unrelated to your current work — create a markdown file in `issues/` with YAML frontmatter (`priority: P2`, `state: new`) and a description. This includes bugs you notice, UI problems, technical debt, requirements you skip, and improvements you spot. Don't let things slip through the cracks.

## Directory Structure

```
issues/          Active issues (new, approved, changes-requested, needs-replan)
review/          Plans awaiting human review
.coven/
  agents/        Agent prompt templates
  config.toml    Optional configuration (entry_agent)
  workflow.md    This file
```
