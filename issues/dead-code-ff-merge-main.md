---
priority: P2
state: new
---

# Dead code: `ff_merge_main` is unused in production

`src/worktree.rs:422-441` defines `ff_merge_main()` — a standalone fast-forward merge step. It was presumably created for a conflict resolution flow where rebase and ff-merge were separate steps, but the current landing logic in `worker.rs` calls the full `land()` function (which includes both rebase + ff-merge) on every attempt. The `land_or_resolve` function's doc comment (worker.rs:663-668) explicitly explains this design: "retries the full land (rebase + ff-merge) rather than just ff-merge."

The function is only exercised by `worktree::tests::ff_merge_after_manual_conflict_resolution` — no production code path calls it.

## Options

1. **Remove it**: delete the function and its test. The test for manual conflict resolution can be done through `land()` after aborting/resolving.
2. **Keep it**: if there's a future plan to optimize conflict resolution by skipping redundant rebase after a successful resolution, mark it with a comment explaining the intent.
