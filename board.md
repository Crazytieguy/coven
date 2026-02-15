# Board

## P1: Investigate: some claude sessions don't get displayed by coven

Maybe listening to the wrong session or something weird like that.

Investigated the full event pipeline: spawning → stdout reading → parsing → event channel → renderer.

**Finding: stderr is completely suppressed.** `runner.rs:75` sets `.stderr(Stdio::null())`, so if the claude CLI encounters an error (auth failure, API rate limits, invalid args, model unavailable), the error goes to stderr (invisible) and the process exits with no stdout. The user sees only "Claude process exited" with zero context.

This is the most likely cause of "sessions that don't get displayed" — the claude process fails before producing any stream-json output, and the error message is swallowed.

**Other things I checked (less likely):**
- Event channel replacement (`io.replace_event_channel()`) — sequential design prevents lost events
- `InboundEvent` enum has no `#[serde(other)]` fallback, but unknown types show as parse warnings (visible, not silent)
- `tokio::select!` fairness in `next_event()` — can delay events but not lose them
- `--verbose` flag is always passed (required for `stream_event` display) — no conditional paths
- Renderer doesn't suppress content under normal conditions

**Proposed fix:** Capture stderr instead of null'ing it. When the process exits, if there's stderr content, display it as a warning. Something like:
```
[warn] Claude stderr: <error message>
Claude process exited
```

**Questions:**
- Does this match what you're observing? (sessions that seem to start but show nothing before exiting?)
- Or is it more like sessions that run for a while and produce output that disappears?

## P1: Review: is `git reset --hard main` correct in the review agent?

Reviewed `review.md`, `land.sh`, `worktree.rs`, and the agent rendering pipeline.

**Finding 1: Hardcoded "main" is wrong.** The review agent hardcodes `main` in two places:
- Line 19: `git diff main...HEAD`
- Line 31: `git reset --hard main`

But `land.sh` and `worktree.rs` both discover the main branch dynamically via `git worktree list --porcelain`. If the main worktree is on `master` or another branch, these commands break.

**Finding 2: `--hard` is appropriate.** The push-back flow is "discard everything, post to board, commit, land." There's nothing to preserve — `--hard` is the right tool. A softer reset would leave uncommitted changes that interfere with the board commit + land.

**Finding 3: `Bash(git reset:*)` permission is fine.** The review agent already has `git rebase:*`, `git rm:*`, and `bash .coven/land.sh` — all equally destructive. The real safety boundary is that the agent only operates in its worktree; `land.sh` handles the main worktree interaction carefully. Tightening `git reset:*` to `git reset:--hard *` doesn't meaningfully reduce risk.

**Proposed fix:** Replace hardcoded `main` with the output of `land.sh`'s branch discovery, or teach the agent to use the main branch from Claude Code's injected `gitStatus` (which includes `Main branch: <name>`). Simplest approach: add a small `main-branch.sh` helper script, or just inline the git command in the prompt. Or we could template it as a variable rendered at agent-dispatch time.

**Questions:**
- Preferred approach? Options:
  1. Tell the agent to read the main branch from its `gitStatus` context (zero code changes, but fragile — relies on Claude Code's format)
  2. Add a `.coven/main-branch.sh` helper that outputs the branch name
  3. Inject `{{main_branch}}` as a template variable at dispatch time (cleanest, but requires code changes to pass the value)
- Also: `git diff main...HEAD` on line 19 has the same problem — should fix both together

---

## P1: Propose a new board format that replaces the divider

"The divider" seems to be confusing. Propose a board format that makes it more obvious what's blocking on human input and what isn't (blocking still at the top).

*In progress (prime-cedar-53)*

## Done
- P1: Split main into main + review agents
- P1: First typed character after entering interactive with Ctrl+O seems to be swallowed
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
- P1: wait-for-user re-proposal
- P1: Simplify status line after exiting embedded interactive session
- P1: wait-for-user prompt final revision
