This session is unattended (no human at the keyboard). One small action at a time — pick the highest-priority action available, do it, and stop. Stopping means outputting a final assistant message with no tool calls — the session ends but the ralph loop continues.

## Priorities

1. **Lint**: Run clippy and fix any warnings.
2. **Plan an unplanned issue**: Pick an issue from issues.md that has no `(plan: ...)` reference. Write a plan file and link it. Planning counts as one action.
3. **Act on reviewed plans**: Read each plan file referenced from issues.md.
   - `Status: approved` — implement the plan. If necessary, re-record relevant VCR tests and regenerate snapshots, and verify that the snapshot reflects the intended change. When done, remove the issue from issues.md and delete the plan file.
   - `Status: rejected` — revise the plan based on the Review section comments. Counts as one action.
   - `Status: draft` — not yet reviewed, skip.
4. **Review test cases**: Don't break early just because higher-priority work is done — these matter too. Review the test cases (case definition, vcr file, snapshot): does each test cover the right thing? Does the vcr recording reflect intended behavior? Is the snapshot correct, readable, and showing good UI? This is also an opportunity to spot UI improvements — if the output could look better, that's an issue. If you find a problem, add it to issues.md and stop.
5. **Add test coverage**: Look for untested features and add VCR + snapshot test cases for them.
6. **Refactor**: Look at the code for refactoring opportunities — there are likely opportunities to make the code cleaner and more DRY, and there are potentially cut corners hiding inside the code.

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

After creating the plan file, update the issue in issues.md to include `(plan: plans/<filename>.md)`.

Questions are optional but encouraged — surface ambiguity rather than guessing.

## Session discipline

- One action, then stop.
- Always verify clippy + tests pass before committing.
- Commit all artifact changes (issues.md, plan files) alongside code changes.
- If your changes introduced new problems, revert and stop. End result should be 0-1 atomic changes, with state at least as clean as when you started.
Good luck!
