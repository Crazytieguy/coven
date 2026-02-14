---
description: "Implements, reviews, and lands work for a board issue"
args:
  - name: task
    description: "Board entry title"
    required: true
claude_args:
  - "--allowedTools"
  - "Bash(git status),Bash(git log:*),Bash(git diff:*),Bash(git add:*),Bash(git mv:*),Bash(git rm:*),Bash(git commit:*),Bash(git rebase:*),Bash(bash .coven/land.sh)"
---

Work on the board issue: **{{task}}**

## Steps

1. Read `board.md` to find your issue entry, and `scratch.md` if it exists for notes from previous sessions
2. Implement the next piece of work — one focused, atomic change
3. Run tests and fix any failures your change introduces
4. Run the linter and fix warnings
5. Commit with a descriptive message
6. Update `scratch.md` with what you did and what's next
7. Self-transition to continue — repeat from step 1 until the issue is fully complete

The number of steps varies by issue. Small tasks may complete in one session; larger ones take several.

## Final Session: Review & Land

When all implementation is done:

1. Review the full diff: `git diff main...HEAD`
2. Do cleanup or fixes if needed (commit any changes)
3. Run `bash .coven/land.sh` — if conflicts, resolve and run again
4. Remove the entry from `board.md`, commit, and run `bash .coven/land.sh` again
5. Delete `scratch.md`
6. Transition to dispatch

## Questions

If at any point you encounter ambiguity — stop. Do not guess at architectural choices, API design, or behavior that isn't explicitly described in the task and its decisions.

Instead:
1. Discard your uncommitted code changes
2. Add questions to your board entry and move it above the divider
3. Commit the board change and run `bash .coven/land.sh`
4. Delete `scratch.md`
5. Transition to dispatch

Code is cheap. Getting things wrong is expensive.

## Recording Issues

If you notice unrelated problems (bugs, tech debt, improvements), add a new entry to `board.md` below the divider with an appropriate priority. Don't stop your current work to fix them.

## Rules

- **Always land before transitioning to dispatch.** The worktree must not be ahead of main. If `land.sh` fails due to conflicts, resolve them and run it again.
- Delete `scratch.md` on every land.
