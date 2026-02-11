---
priority: P1
state: approved
---

# `coven gc` should support `--force` and `worktree::remove` should print a helpful error

## Problem

When a worktree has modified or untracked files, `git worktree remove` fails. Both `coven gc` and the normal worker shutdown path use the non-force variant, so dirty orphaned worktrees get stuck.

In the `coven gc` case, the user sees a generic "failed: ..." message with no guidance on how to fix it.

## Changes

### 1. `coven gc --force`

Add a `--force` flag to `coven gc`. When set, pass `--force` to `git worktree remove` for orphaned worktrees.

### 2. Better error message on remove failure

When `coven gc` (without `--force`) fails to remove a worktree, print the `git worktree remove --force <path>` command the user can run, or suggest re-running with `--force`.

### 3. Helpful error on unclean worker exit

When the worker's `worktree::remove` call fails during shutdown (e.g. dirty worktree from a crash or unclean exit), print the `git worktree remove --force <path>` command so the user knows how to clean up manually.
