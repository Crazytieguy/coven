Issue: [P0] I want to step up the workflow command testing: have a test with `coven init` + multiple `coven worker` running concurrently, test a real situation
Status: draft

## Approach

Add a new multi-command VCR test type that orchestrates `coven init` + two concurrent `coven worker` instances in a single test case, against a shared git repo with realistic issues.

### Infrastructure changes

**New test case type: `[multi]`** — a TOML section that defines a sequence of named steps:

```toml
[multi]
steps = [
  { name = "init", command = "init", stdin = "y" },
  { name = "setup", command = "shell", script = "create-issues.sh" },
  { name = "worker_a", command = "worker", concurrent_group = "workers" },
  { name = "worker_b", command = "worker", concurrent_group = "workers" },
]
```

Steps without `concurrent_group` run sequentially. Steps sharing a `concurrent_group` run concurrently. Each step that involves a VCR-recording command gets its own VCR file (`<name>__<step>.vcr`).

**Recording flow:**
1. Create temp git repo with `[files]` and initial commit (same as today)
2. Run steps in order. Sequential steps complete before the next group starts. Concurrent steps are spawned together via `tokio::task::spawn_local`.
3. Each command step records to its own VCR file. Shell steps execute without VCR.
4. Each command step captures output to its own buffer for snapshot comparison.

**Replay flow:**
1. Load all VCR files for the test case
2. Run steps in the same sequence/concurrency structure
3. Each command replays from its own VCR context
4. Snapshot: concatenate all step outputs with headers (`--- worker_a ---`, `--- worker_b ---`)

### Concurrency realism

During **recording**, the concurrent workers interact through real shared state: dispatch lock, worker state files, git worktrees, and the shared repo. This is where the concurrency logic is validated.

During **replay**, each worker replays its own VCR tape independently. The concurrency paths (lock blocking, state file reads) return pre-recorded values. Replay validates that the code paths don't crash and output matches expectations — it's a regression test, not a concurrency test. The real concurrency validation happens at recording time.

### Test scenario

A realistic multi-worker scenario with two issues:

**Setup files:**
- `.coven/agents/dispatch.md` — dispatch logic that reads `issues/` and assigns work
- `.coven/agents/fix-readme.md` — agent that fixes a README typo
- `.coven/agents/add-tests.md` — agent that adds a test file
- `issues/fix-readme.md` — first issue
- `issues/add-tests.md` — second issue
- `.claude/settings.json` — permissions for git operations

**Expected flow:**
1. Init step creates the standard `.coven/` structure (or we skip init and provide files directly via `[files]`)
2. Two workers start concurrently
3. Worker A dispatches first (acquires lock), picks `fix-readme`
4. Worker B dispatches second, picks `add-tests`
5. Both complete their agents, land their changes
6. Both dispatch again, see no remaining issues, sleep
7. Both exit via trigger on `main_head_sha` label (same as `worker_basic`)

### Shell steps for mid-test setup

Some scenarios need filesystem changes between commands (e.g., creating issue files after init). The `shell` step type runs a script from `[scripts]` in the TOML against the test repo:

```toml
[scripts]
"create-issues.sh" = '''
mkdir -p issues
echo "Fix the typo in README" > issues/fix-readme.md
echo "Add unit tests for parser" > issues/add-tests.md
git add . && git commit -m "Add issues"
'''
```

This avoids needing a complex init VCR recording just to set up the test state. We can provide the `.coven/` structure via `[files]` and use a shell step for any post-init-commit setup.

## Questions

### Should we skip init and set up files directly?

Running `coven init` as part of the test adds a third VCR recording and more complexity. Since init is already tested by `init_fresh`, we could provide the `.coven/` directory structure via `[files]` and focus the test on the concurrent worker behavior.

Option 1: **Skip init, provide files** — simpler, focused on concurrent workers
Option 2: **Include init step** — tests full user flow, more realistic

Answer:

### How many concurrent workers?

The issue says "multiple", which could mean 2 or more. Two workers is the simplest case that exercises dispatch lock contention and demonstrates concurrent behavior.

Option 1: **Two workers** — minimum viable for concurrency testing, simpler to debug
Option 2: **Three workers** — one worker will need to wait for dispatch, exercises more contention

Answer:

### Should workers experience a land conflict?

The most interesting concurrent scenario is when two workers both have changes and one needs to rebase. We could set up the scenario so both workers modify overlapping files, forcing conflict resolution.

Option 1: **No conflicts** — each worker modifies different files, clean lands. Tests the happy path of concurrent dispatch + landing.
Option 2: **With conflict** — workers modify overlapping files, second to land must resolve. Tests the conflict resolution path. Significantly more complex to record.

Answer:

## Review

