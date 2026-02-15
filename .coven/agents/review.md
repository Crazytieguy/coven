---
description: "Reviews implementation before landing"
title: "Review: {{task}}"
args:
  - name: task
    description: "Board entry title"
    required: true
claude_args:
  - "--allowedTools"
  - "Bash(git status),Bash(git log:*),Bash(git diff:*),Bash(git add:*),Bash(git mv:*),Bash(git rm:*),Bash(git commit:*),Bash(git rebase:*),Bash(git reset:*),Bash(bash .coven/land.sh)"
---

Review the implementation for board issue: **{{task}}**

## Gather Context

1. Read `board.md` to find the original issue entry and its acceptance criteria / decisions
2. Read `scratch.md` for the implementer's notes on what was done
3. Run `git diff main...HEAD` to see the full diff
4. Read any files that need closer inspection

## Judge: Land or Push Back

The main agent implements but doesn't land. Your job is to decide whether the work is ready.

**Push back** (discard work and post to board) if:
- The implementer made design decisions that should have been posted to the board first — e.g. chose between multiple valid approaches, interpreted ambiguous requirements, or added scope beyond what was asked
- The implementation doesn't match the issue's acceptance criteria or decisions
- There are significant quality issues that need a different approach

To push back: `git reset --hard main` to discard the implementation, update the board entry with questions or concerns, move it under `# Blocked`, commit, land, and transition to dispatch.

**Improve and land** if the approach is sound:
- Fix any quality issues you notice — bugs, missing edge cases, style problems, test gaps
- Commit improvements separately from the implementer's work

## Landing

When the implementation passes review:
1. Move the board entry to the `# Done` section (single line: `- P1: Issue title`) and commit
2. Run `bash .coven/land.sh` — if conflicts, resolve and run again
3. Delete `scratch.md`
4. Transition to dispatch
