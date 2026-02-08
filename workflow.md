This session is unattended (no human at the keyboard). One small action at a time — pick the highest-priority action available, do it, then end your response. The loop will continue with a new session automatically.

## Priorities

1. **Lint**: Run clippy and fix any warnings.
2. **Work on issues**: Process issues from `issues.md` by priority (`[P0]` > `[P1]` > `[P2]`; untagged defaults to `[P1]`). Within the same priority level, prefer planning over implementing. Across levels, implementing a higher-priority issue takes precedence over planning a lower-priority one.
   - **Plan**: Pick the highest-priority issue that has no `(plan: ...)` reference. Write a plan file and link it. Planning counts as one action.
   - **Implement**: Check git status (or the gitStatus provided at session start) for modified plan files — uncommitted modifications to plan files mean the human has reviewed them. Read those files and act on their updated status. As a low-priority fallback, also read all plan files referenced from issues.md if git-based discovery found nothing.
     - `Status: approved` — implement the plan. If necessary, re-record relevant VCR tests and regenerate snapshots, and verify that the snapshot reflects the intended change. When done, remove the issue from issues.md and delete the plan file.
     - `Status: rejected` — revise the plan based on the Review section comments. Counts as one action.
     - `Status: draft` — not yet reviewed, skip.
3. **Review test cases**: Don't break early just because higher-priority work is done — these matter too. Review the test cases (case definition, vcr file, snapshot): does each test cover the right thing? Does the vcr recording reflect intended behavior? Is the snapshot correct, readable, and showing good UI? This is also an opportunity to spot UI improvements — if the output could look better, that's an issue. If you find a problem, add it to issues.md and stop.
4. **Add test coverage**: Look for untested features and add VCR + snapshot test cases for them.
5. **Refactor**: Look at the code for refactoring opportunities — there are likely opportunities to make the code cleaner and more DRY, and there are potentially cut corners hiding inside the code.

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

After any code-changing action, spawn a review subagent (Task tool, general-purpose type) to review the changes. The prompt should be a single sentence: "Review the uncommitted changes in this repo and surface anything that could be improved — only approve if everything is pristine." If the review surfaces issues, fix them and re-review. Only proceed to commit once the review returns clean.

## Session discipline

- One action, then end your response.
- Always verify clippy + tests pass before committing.
- After creating or modifying a plan file (and updating issues.md if needed), commit the plan changes immediately — don't wait to bundle them with code changes.
- Commit all artifact changes (issues.md, plan files) alongside code changes.
- If your changes introduced new problems, revert and stop. End result should be 0-1 atomic changes, with state at least as clean as when you started.
Good luck!
