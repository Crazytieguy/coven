---
description: "Implements, reviews, and lands work for a board issue"
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

## Each Session

1. Read `board.md` to find your issue entry, and `scratch.md` if it exists for context from previous sessions
2. Do one focused piece of work toward the issue. Commit.
   - **First session** (no `scratch.md`): focus on understanding — read relevant code, identify questions, plan your approach. Start coding only if the task is straightforward and unambiguous.
3. Update `scratch.md` with what you did and what's next

## Between Sessions

If more implementation work remains, self-transition to continue.

When implementation is complete, self-transition for a **review session**: review the full diff (`git diff main...HEAD`), verify acceptance criteria, and fix anything that needs it.

When review passes:
1. Move the entry to the `## Done` section of `board.md` (single line: `- P1: Issue title`) and commit
2. Run `bash .coven/land.sh` — if conflicts, resolve and run again
3. Delete `scratch.md`
4. Transition to dispatch

## Questions

If at any point you encounter ambiguity — stop. Do not guess at architectural choices, API design, or behavior that isn't explicitly described in the task and its decisions.

More broadly: when you see multiple viable approaches — even for small decisions — prefer asking over choosing. If you'd mention "I went with X" in a scratch note, that's a sign you should ask first. The cost of a round-trip is low; the cost of rework is high.

To ask questions:
1. Discard your un-landed code changes
2. Add questions to your board entry and move it above the divider
3. Commit the board change
4. Land (`bash .coven/land.sh`) and transition to dispatch

Keep board entries concise. Questions need only enough context for a human to answer — not design explorations, approach comparisons, or code examples. A good question entry is 2-5 lines.

Code is cheap. Getting things wrong is expensive.

## Recording Issues

If you notice unrelated problems (bugs, tech debt, improvements), add a new entry to `board.md` below the divider with an appropriate priority. Don't stop your current work to fix them.

## Rules

- **Never transition to dispatch without landing first.** The worktree must not be ahead of main.
- Delete `scratch.md` on every land.
