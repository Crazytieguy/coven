We recently finished the initial implementation of this project. I want to work through improving the project, one small session at a time. This is a non-interactive session. Pick one task from the list below (in priority order), do it, and stop. Stopping means outputting a final assistant message with no tool calls — the session ends but the ralph loop continues.

1. Run clippy and ensure there are no warnings (should be clean).
2. Choose a non-blocked issue from issues.md to tackle. If the issue has a `(see questions/X.md)` reference, read the answered questions first. If necessary re-record relevant vcr tests and regenerate snapshots, and verify that the snapshot looks as expected. When an issue is resolved, remove it from the list and delete any associated question file.
3. Review the test cases (case definition, vcr file, snapshot) for issues: does each test cover the right thing? Does the vcr recording reflect intended behavior? Is the snapshot correct, readable, and showing good UI? This is also an opportunity to spot UI improvements — if the output could look better, that's an issue. If you find a problem, add it to issues.md and stop.
4. If you notice a feature that has no test coverage, add a test case for it.
5. Look at the code for refactoring opportunities: there are likely opportunities to make the code cleaner and more dry, and there are potentially cut corners hiding inside the code.

Whichever task you pick, finish by verifying that linting is no worse than it was when you started and all tests pass, then commit (autonomously). Always commit changes to issues.md, question files, and other non-code artifacts too. If your changes introduced new problems, just revert and stop. End result should be 0-1 atomic changes to the codebase, with state at least as clean as when you started.

At the end of each session, check `questions/` for answered questions — files where every `Answer:` line has a response. For each fully-answered file: update the issue in issues.md from `(blocked on questions/X.md)` to `(see questions/X.md)` to indicate it's now actionable, and commit.

## Design questions

If you encounter a design question that needs human input, don't guess — record it and stop:

1. Every question must be tied to an issue. If the thing you're working on isn't already in issues.md, add it first.
2. Create a file in `questions/` with a descriptive kebab-case name (e.g., `questions/subagent-display.md`). A single file can contain multiple questions — keep all questions about one issue together. Format:
   ```
   Blocks: <copy of the issue text from issues.md>

   ## <question 1>
   <context, options, tradeoffs — enough for the human to make a decision>

   Answer:

   ## <question 2>
   ...

   Answer:
   ```
3. Mark the related issue in issues.md as blocked: `(blocked on questions/filename.md)`
Recording a design question counts as work for the session.

## When to break

Use `<break>reason</break>` to end the ralph loop when there's genuinely nothing productive left to do:
- All remaining issues in issues.md are blocked on unanswered questions (or there are no issues)
- All test cases look correct, well-structured, and show good UI
- No untested features found
- No refactoring opportunities found

Good luck!
