#!/usr/bin/env bash
set -euo pipefail

# Check we're in a git repo
if ! git rev-parse --git-dir >/dev/null 2>&1; then
    echo "Error: not in a git repository" >&2
    exit 1
fi

# Get current worktree path and branch
current_path=$(git rev-parse --show-toplevel)
current_branch=$(git rev-parse --abbrev-ref HEAD)

if [[ "$current_branch" == "HEAD" ]]; then
    echo "Error: worktree is in detached HEAD state" >&2
    exit 1
fi

# Find main worktree
worktree_list=$(git worktree list --porcelain)
main_path=$(echo "$worktree_list" | head -1 | sed 's/^worktree //')
main_branch=$(echo "$worktree_list" | grep -m1 '^branch ' | sed 's/^branch refs\/heads\///')

if [[ -z "$main_branch" ]]; then
    echo "Error: could not parse git worktree list output" >&2
    exit 1
fi

if [[ "$current_path" == "$main_path" ]]; then
    echo "Error: already in the main worktree — run from a secondary worktree" >&2
    exit 1
fi

# Check for uncommitted changes
if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
    echo "Error: uncommitted changes — commit or stash before landing" >&2
    exit 1
fi

# Rebase onto main
commit_count=$(git rev-list --count "$main_branch..$current_branch" 2>/dev/null || echo "0")

if [[ "$commit_count" -eq 0 ]]; then
    echo "No commits to land (branch is up to date with $main_branch)"
    exit 0
fi

echo "Rebasing $commit_count commit(s) onto $main_branch..."

if ! rebase_output=$(git rebase "$main_branch" 2>&1); then
    conflicting_files=$(git diff --name-only --diff-filter=U 2>/dev/null || true)
    if [[ -n "$conflicting_files" ]]; then
        echo "Rebase has conflicts in:" >&2
        echo "$conflicting_files" | sed 's/^/  /' >&2
        echo "" >&2
        echo "To resolve:" >&2
        echo "  1. Fix the conflicts in the files above" >&2
        echo "  2. git add <resolved-files>" >&2
        echo "  3. git rebase --continue" >&2
        echo "  4. Run bash .coven/land.sh again" >&2
    else
        echo "$rebase_output" >&2
    fi
    exit 1
fi

# Fast-forward main in the main worktree
cd "$main_path"
if ! git merge --ff-only "$current_branch" 2>&1; then
    echo "Error: could not fast-forward $main_branch to $current_branch" >&2
    echo "Main may have new commits. Try rebasing again." >&2
    exit 1
fi

echo "Landed $commit_count commit(s) on $main_branch"
