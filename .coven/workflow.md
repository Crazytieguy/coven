# Orchestration Workflow

This project uses [coven](https://github.com/yoavshapira/coven) for orchestrated development. Multiple workers run simultaneously, each picking up tasks from the issue queue.

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

## Reviewing Plans

Plans appear in `review/`. To review one:

1. Read the `## Plan` section and any `## Questions`
2. Answer questions inline (fill in below `**Answer:**` markers)
3. Update frontmatter: `state: approved` or `state: changes-requested`
4. Move the file from `review/` back to `issues/`
5. Commit

There's no time pressure — workers will wait or work on other issues.

## Recording Issues

Always record issues you encounter that are unrelated to your current work — create a markdown file in `issues/` with YAML frontmatter (`priority: P2`, `state: new`) and a description. This includes bugs you notice, UI problems, technical debt, requirements you skip, and improvements you spot. Don't let things slip through the cracks.

## Directory Structure

```
issues/          Active issues (new, approved, changes-requested, needs-replan)
review/          Plans awaiting human review
.coven/
  agents/        Agent prompt templates
  workflow.md    This file
```
