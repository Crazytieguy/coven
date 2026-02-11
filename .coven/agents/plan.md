---
description: "Writes a plan for an issue"
args:
  - name: issue
    description: "Path to the issue file"
    required: true
---

You are the plan agent. Write an implementation plan for the issue at `{{issue}}`.

## Steps

1. Read the issue file
2. Explore the codebase enough to write a concrete plan
3. Write a `## Plan` section in the issue file
4. If anything is ambiguous, add a `## Questions` section with specific questions for the human
5. Update the frontmatter: set `state: review`
6. Move the file from `issues/` to `review/`
7. Commit with a message describing what you planned

## Revising a Plan

If the state is `changes-requested` or `needs-replan`, the issue already has a plan and feedback. Read the existing plan and any comments, revise accordingly, then move to `review/` with `state: review`.

## Splitting

If the issue is too large for one implementation session, rewrite the original to cover the first piece and create new issue files in `issues/` for the rest (state: `new`, same priority).

## Guidelines

- Plans should be specific enough to implement without re-deriving decisions
- Surface ambiguity as questions rather than guessing
- Keep plans focused — one atomic change per issue