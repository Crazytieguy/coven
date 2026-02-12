---
priority: P2
state: review
---

# Investigate race condition: commits on main missed during sleep setup

When a worker decides to sleep and starts watching main refs for new commits, there may be a window where a commit lands after the agent started running (so it sees stale state) but before the watcher is active — causing the worker to sleep indefinitely and miss the commit.

Investigate whether this race exists, and if so propose ways to handle it.

## Investigation

The race exists. Here's the timeline:

1. `worker_loop` calls `sync_to_main()` — worktree is rebased, main HEAD = A
2. `run_agent_chain()` starts — dispatch reads issues from the filesystem (state at commit A)
3. While the chain runs, another worker lands commit B on main (e.g. adding a new issue)
4. Dispatch sees no actionable work (based on stale state from A) → outputs `<next>sleep: true</next>`
5. `wait_for_new_commits()` is called
6. Ref watcher is set up
7. `initial_head = vcr_main_head_sha(...)` reads current main HEAD = **B** (already moved!)
8. Worker waits for HEAD to differ from B — but the work introduced by commit B was never evaluated by dispatch

The worker sleeps until yet another commit arrives. In a low-activity project this could be a long wait.

Note: `wait_for_new_commits` already handles the TOCTOU race *within itself* correctly (watcher set up before HEAD read at line 696-700). The problem is that the "baseline" for comparison should be the state dispatch actually saw, not the state at sleep time.

## Plan

Pass the pre-chain main HEAD SHA into `wait_for_new_commits` so it compares against the state the dispatch agent actually observed, rather than the (possibly stale) state at sleep entry.

### Changes in `src/commands/worker.rs`

1. **Capture pre-chain HEAD in the outer loop** (around line 219, after `sync_to_main`):
   ```rust
   let pre_chain_head = vcr_main_head_sha(vcr, wt_str.clone()).await?;
   ```

2. **Pass `pre_chain_head` into `wait_for_new_commits`** (around line 238):
   ```rust
   let wait = wait_for_new_commits(
       worktree_path, &pre_chain_head, ctx.renderer, ctx.input, ctx.io, ctx.vcr,
   );
   ```

3. **Update `wait_for_new_commits` signature** to accept `pre_chain_head: &str` instead of reading `initial_head` internally:
   - Remove the `let initial_head = vcr_main_head_sha(...)` call (line 700)
   - Use `pre_chain_head` in all comparisons instead of `initial_head`
   - This means after the watcher is set up (line 698), if main already moved during the chain, the first `vcr_main_head_sha` check in the select loop will detect the difference immediately and return `NewCommits`

### Behavior change

- **Before:** Worker sleeps until a commit lands *after* `wait_for_new_commits` is entered
- **After:** Worker detects any commit that landed *during* the agent chain run and immediately re-dispatches

### Edge cases

- If main moved during the chain but the new commit is irrelevant (e.g. only touches unrelated files), dispatch will re-run, see nothing actionable, and sleep again. This is a harmless extra dispatch cycle.
- The existing TOCTOU protection within `wait_for_new_commits` (watcher before HEAD read) still works — commits landing between watcher setup and the first SHA check will be caught by the watcher.

### No new tests needed

The existing VCR test infrastructure handles the sleep/wake flow. The change is a one-line semantic shift (comparing against an earlier SHA) that doesn't introduce new branches or error paths.
