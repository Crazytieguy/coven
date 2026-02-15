---
description: "Works on a board issue"
title: "{{task}}"
args:
  - name: task
    description: "Board entry title"
    required: true
claude_args:
  - "--allowedTools"
  - "Bash(git status),Bash(git log:*),Bash(git diff:*),Bash(git add:*),Bash(git mv:*),Bash(git rm:*),Bash(git commit:*),Bash(git rebase:*),Bash(bash .coven/land.sh)"
---

Work on the board issue: **{{task}}**

## Orient

1. Read `board.md` to find your issue entry
2. Read `scratch.md` if it exists for context from previous sessions
3. Read relevant code to understand the problem

## Decide

Not every task requires code changes. Read the task carefully and choose:

- **Post to the board** — when the task asks you to propose, research, or analyze; when you encounter ambiguity; or when multiple approaches are viable. Update your board entry with findings and a question (even if just "good to proceed?"), move it above the divider, commit, land, and transition to dispatch. When in doubt, prefer this — if you'd mention "I went with X" in a scratch note, that's a sign you should post to the board first.
- **Implement** — when the task unambiguously asks for code changes and the path forward is clear. Do one focused piece of work, commit, and update `scratch.md` with what you did and what's next.

If you start implementing and hit ambiguity or a decision point, stop — discard your un-landed code changes and post to the board instead.

Code is cheap. Getting things wrong is expensive.

## Implementation Sessions

If more work remains, transition to main again to continue.

When implementation is complete, transition to main again for a **review session**: review the full diff (`git diff main...HEAD`), verify acceptance criteria, and fix anything that needs it.

When review passes:
1. Move the entry to the `## Done` section of `board.md` (single line: `- P1: Issue title`) and commit
2. Run `bash .coven/land.sh` — if conflicts, resolve and run again
3. Delete `scratch.md`
4. Transition to dispatch

## Recording Issues

If you notice unrelated problems (bugs, tech debt, improvements), add a new entry to `board.md` below the divider with an appropriate priority. Don't stop your current work to fix them.

## Rules

- **Land before transitioning to dispatch.** The worktree must not be ahead of main when returning to dispatch.
- Delete `scratch.md` on every land.
