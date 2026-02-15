# Board

## P1: Propose a new board format that replaces the divider

Replace the `---` divider with H1 section headers. The file is already called `board.md`, so the `# Board` title is redundant — use that heading level for semantic sections instead:

```markdown
# Blocked

## P1: Issue with questions

**Questions:**
- Need human input here

# Ready

## P2: Another issue

Description.

# Done

- P1: Completed issue
- P2: Another completed issue
```

**Why this works:**
- Self-documenting — "Blocked" and "Ready" say exactly what the divider couldn't
- Minimal change — issues stay as H2, Done stays the same, just swap the divider for named H1 sections
- Easy to reference in prompts — "move to `# Blocked`" / "move to `# Ready`" is clearer than "above/below the divider"

**What changes in the prompts:**
- `system.md`: new format example, replace divider language with section names
- `dispatch.md`: "below the divider" → "under `# Ready`", "above the divider" → "under `# Blocked`"
- `main.md`, `review.md`: same substitutions
- `init.rs`: brief template reference

**Questions:**
- Good to proceed with this approach?
- Any preference on the section names? (`Blocked`/`Ready` vs `Waiting`/`Active` vs something else)

---

## P1: Review: is `git reset --hard main` correct in the review agent?

Reviewed `review.md`, `land.sh`, `worktree.rs`, and the agent rendering pipeline.

**Finding 1: Hardcoded "main" is wrong.** The review agent hardcodes `main` in two places:
- Line 19: `git diff main...HEAD`
- Line 31: `git reset --hard main`

But `land.sh` and `worktree.rs` both discover the main branch dynamically via `git worktree list --porcelain`. If the main worktree is on `master` or another branch, these commands break.

**Finding 2: `--hard` is appropriate.** The push-back flow is "discard everything, post to board, commit, land." There's nothing to preserve — `--hard` is the right tool. A softer reset would leave uncommitted changes that interfere with the board commit + land.

**Finding 3: `Bash(git reset:*)` permission is fine.** The review agent already has `git rebase:*`, `git rm:*`, and `bash .coven/land.sh` — all equally destructive. The real safety boundary is that the agent only operates in its worktree; `land.sh` handles the main worktree interaction carefully. Tightening `git reset:*` to `git reset:--hard *` doesn't meaningfully reduce risk.

**Decisions:**
- Approach: Add context about the main branch to the coven worker system prompt, so the agent reads it from its `gitStatus` context (which includes `Main branch: <name>`). Since coven worker is built around worktrees, this fits naturally.
- Permissions: ok to keep as-is
- Fix both `git diff main...HEAD` and `git reset --hard main` together

## P1: Investigate: some claude sessions don't get displayed by coven

Coven hangs and doesn't display, but claude is actually running in the background. Not a session-exit issue — the process is alive.

**Decisions:**
- Sessions don't exit — coven hangs and doesn't display, but claude is actually running in the background. The original stderr hypothesis was wrong for this issue.
- The stderr fix (capturing stderr instead of null'ing it) is good but is a separate issue — already done.

**New direction:** The problem is that coven's display layer stops showing output even though the claude process is still running. Need to investigate the streaming/rendering pipeline for cases where events are received but not displayed, or where stdout reading stalls.

Previous investigation ruled out: event channel replacement, serde fallback, tokio::select fairness, --verbose flag, renderer suppression.

## Done
- P2: Capture stderr from claude process
- P1: Split main into main + review agents
- P1: First typed character after entering interactive with Ctrl+O seems to be swallowed
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
- P1: wait-for-user re-proposal
- P1: Simplify status line after exiting embedded interactive session
- P1: wait-for-user prompt final revision
