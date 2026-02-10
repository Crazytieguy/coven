This session is unattended (no human at the keyboard). One small action at a time — pick the highest-priority action available, do it, then end your response. The loop will continue with a new session automatically.

## Priorities

1. **Lint**: Run clippy and fix any warnings.
2. **Work on issues**: Process issues from `issues.md` by priority (`[P0]` > `[P1]` > `[P2]`; untagged defaults to `[P1]`). Within the same priority level, prefer planning over implementing. Across levels, **implementing a higher-priority issue takes precedence over planning a lower-priority one**.
   - **Plan**: Pick an issue that has no `(plan: ...)` reference. Write a plan file and link it. Planning counts as one action.
   - **Implement**: Check git status (or the gitStatus provided at session start) for modified plan files — uncommitted modifications to plan files mean the human has reviewed them. Read those files and act on their updated status.
     - `Status: approved` — implement the plan. If necessary, re-record relevant VCR tests and regenerate snapshots, and verify that the snapshot reflects the intended change. When done, remove the issue from issues.md and delete the plan file.
     - `Status: rejected` — revise the plan based on the Review section comments. After revising, clear the Review section and any inline notes so the human knows it needs re-review. Counts as one action.
     - `Status: draft` — not yet reviewed, skip.
3. **Audit the codebase**: Identify suboptimal snapshots, messy code, small or large scale duplication, untested features, or anything else that can be improved. If the fix is obvious: fix immediately. If non-obvious/there are tradeoffs, add an issue to issues.md

## Writing plans

Create `plans/<issue-name>.md` using kebab-case. Format:

```
Issue: <copy of the issue one-liner from issues.md>
Status: draft

## Approach

<what to change, where, how — enough detail to implement without re-deriving>

## Questions

### <question title>
<context, options, tradeoffs — enough for the human to decide>

Answer:

## Review

<human writes approval/rejection/comments here>
```

After creating the plan file, update the issue in issues.md to include `(plan: plans/<filename>.md)`. Commit the plan file and issues.md update immediately.

Questions are optional but encouraged — surface ambiguity rather than guessing.

## Review before committing

After any code-changing action, spawn a review subagent (Task tool, general-purpose type) to review the changes. The prompt should be a single sentence: "Review the uncommitted changes in this repo and surface anything that could be improved — only approve if everything is pristine. Ignore changes to plans/ and issues.md — these are workflow artifacts, not code." If the review surfaces issues, fix them and re-review. Only proceed to commit once the review returns clean.

## Session discipline

- One action, then end your response.
- Always verify clippy + tests pass before committing.
- After creating or modifying a plan file (and updating issues.md if needed), commit the plan changes immediately — don't wait to bundle them with code changes.
- Commit all artifact changes (issues.md, plan files) alongside code changes.
- If your changes introduced new problems, revert and stop. End result should be 0-1 atomic changes, with state at least as clean as when you started.
Good luck!
