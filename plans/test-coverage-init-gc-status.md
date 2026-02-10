Issue: Missing test coverage for `init`, `gc`, and `status` commands — these have no VCR test cases. All other commands have at least basic coverage.
Status: draft

## Approach

These three commands differ fundamentally from `run`/`ralph`/`worker` — they perform local I/O (filesystem, git commands) but never call the Claude API. The current VCR test harness (`vcr_test.rs`) only handles `run`, `ralph`, and `worker` case types.

### Option A: Extend VCR test harness (recommended)

Refactor the commands to accept a `VcrContext` and route all I/O through `vcr.call()`, matching how `worker` already handles `worker_state::read_all`, `worktree::list_worktrees`, etc. Then extend the test harness with new case types (`[init]`, `[gc]`, `[status]`) and use `record-vcr` to record them.

**init**: Wrap `fs::create_dir_all`, `fs::write`, `fs::read_to_string`, and `stdin.read_line` in VCR calls. Test scenarios:
- Fresh init (all files created)
- Idempotent init (all files skipped)
- CLAUDE.md update prompt (Y/n)

**gc**: Route `worktree::list_worktrees`, `worker_state::read_all`, and `worktree::remove` through VCR. Test scenarios:
- No orphaned worktrees
- One or more orphaned worktrees removed
- Mixed success/failure removal

**status**: Route `worker_state::read_all` through VCR. Test scenarios:
- No active workers
- Multiple workers with various states (idle, active with args)

**Pros**: Consistent with codebase conventions. I/O is fully deterministic. Recording is cheap.
**Cons**: More upfront work to thread VCR context through these commands.

### Option B: Standalone integration tests

Test these commands with real filesystem state in a tempdir, using actual git repos. No VCR recording. Write tests directly in a new `tests/command_tests.rs`.

**Pros**: Simpler, no command refactoring needed.
**Cons**: Tests hit real filesystem/git. Not deterministic for PID-based checks. Inconsistent with other command test patterns.

## Questions

### Testing approach

Option A (extend VCR) is more work but consistent with the codebase pattern of VCR-recording all I/O. Option B is simpler but creates a second testing pattern. Which approach do you prefer?

Answer:

### init stdin interaction

The `init` command reads from stdin for the CLAUDE.md prompt (`[Y/n]`). The VCR test `Io::dummy()` already provides a dummy stdin. Should we:
1. Test with the dummy stdin (which returns empty input, treated as "Y")
2. Extend `Io` to support pre-configured stdin responses for testing

Answer:

## Review

