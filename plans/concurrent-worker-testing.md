Issue: [P0] I want to step up the workflow command testing: have a test with `coven init` + multiple `coven worker` running concurrently, test a real situation
Status: draft

## Approach

Add a multi-step VCR test (`concurrent_workers`) that runs `coven init` → shell setup → two concurrent `coven worker` instances against a shared git repo. Uses the real init templates so the test breaks if they change.

### 1. Multi-step test infrastructure

**New `[multi]` section in TestCase** — defines an ordered list of named steps:

```toml
[multi]
steps = [
  { name = "init", command = "init", stdin = "y" },
  { name = "setup", command = "shell", script = "create-issues.sh" },
  { name = "worker_a", command = "worker", concurrent_group = "workers" },
  { name = "worker_b", command = "worker", concurrent_group = "workers" },
]
```

Step types:
- `init` — runs `coven init` with the given `stdin`. Records to `<test>__<step>.vcr`.
- `shell` — runs a bash script from `[scripts]` against the test repo. No VCR.
- `worker` — runs `coven worker`. Records to `<test>__<step>.vcr`.

Execution model:
- Steps without `concurrent_group` run sequentially in order.
- Steps sharing a `concurrent_group` launch together and all must complete before the next sequential step starts.
- Each VCR-recording step gets its own VCR file and output buffer.

**New structs in vcr.rs:**

```rust
pub struct MultiConfig {
    pub steps: Vec<MultiStep>,
}

pub struct MultiStep {
    pub name: String,
    pub command: String,         // "init", "shell", "worker"
    pub stdin: Option<String>,   // for init
    pub script: Option<String>,  // for shell (key into [scripts])
    pub concurrent_group: Option<String>,
}
```

`TestCase` gains `multi: Option<MultiConfig>` and `scripts: HashMap<String, String>`.

**Changes to record_vcr.rs:**

Add `record_multi_case` alongside the existing `record_case`. Flow:

1. Create temp git repo with `[files]` and initial commit (reuse `setup_test_dir`).
2. Walk steps in order, grouping consecutive steps with the same `concurrent_group`.
3. Sequential steps: run inline, writing VCR files as `<test>__<step>.vcr`.
4. Concurrent group: `tokio::task::spawn_local` each step, join all.
5. Shell steps: `std::process::Command::new("bash").arg("-c").arg(script).current_dir(test_dir)`, with git env vars set. No VCR.
6. Each VCR step captures output to its own buffer.

**Changes to vcr_test.rs:**

Add `multi_vcr_test!` macro (or extend `vcr_test!`). Replay flow:

1. Load all VCR files for the test (`<test>__<step>.vcr` per VCR step).
2. Run steps in the same sequence/concurrency structure, each replaying from its own `VcrContext`.
3. Concatenate outputs with headers (`--- init ---\n`, `--- worker_a ---\n`, etc.).
4. Snapshot the concatenated output.

During replay, each worker replays its own VCR tape independently. Concurrency paths (dispatch lock, state file reads) return pre-recorded values. This is a regression test — the real concurrency validation happens at recording time.

### 2. Template change detection

Make init templates `pub(crate)` (currently `const` in `init.rs`) so tests can access them. Add a unit test in `vcr_test.rs` (or a dedicated test file) that snapshots all init template content:

```rust
#[test]
fn init_templates() {
    insta::assert_snapshot!("dispatch_template", coven::commands::init::DISPATCH_PROMPT);
    insta::assert_snapshot!("plan_template", coven::commands::init::PLAN_PROMPT);
    insta::assert_snapshot!("implement_template", coven::commands::init::IMPLEMENT_PROMPT);
    insta::assert_snapshot!("audit_template", coven::commands::init::AUDIT_PROMPT);
}
```

When a template changes: this test fails immediately, signaling the concurrent worker test needs re-recording. The snapshot diff shows exactly what changed.

### 3. Test scenario: `concurrent_workers`

**TOML:**

```toml
[multi]
steps = [
  { name = "init", command = "init", stdin = "y" },
  { name = "setup", command = "shell", script = "create-issues.sh" },
  { name = "worker_a", command = "worker", concurrent_group = "workers" },
  { name = "worker_b", command = "worker", concurrent_group = "workers" },
]

[files]
".claude/settings.json" = '{"permissions":{"allow":["Bash(git:*)","Bash(ls:*)","Bash(cat:*)"]}}'
"README.md" = "Helo, world!\n"
"src/main.py" = "print(\"hello\")\n"

[scripts]
"create-issues.sh" = '''
cat > issues/fix-typo.md << 'ISSUE'
---
priority: P1
state: approved
---

# Fix README typo

## Plan

Change "Helo" to "Hello" in README.md. No tests or linter needed — text-only change.
ISSUE

cat > issues/add-contributing.md << 'ISSUE'
---
priority: P1
state: approved
---

# Create CONTRIBUTING.md

## Plan

Create a CONTRIBUTING.md file with a brief "how to contribute" section. No tests or linter needed.
ISSUE

git add . && git commit -m "Add issues"
'''

[[messages]]
content = ""
label = "main_head_sha"
mode = "exit"
```

**Expected flow:**
1. `init` creates `.coven/agents/` (dispatch, plan, implement, audit), `issues/`, `review/`, `.coven/workflow.md`, `CLAUDE.md`.
2. `setup` creates two approved issues and commits them.
3. Workers A and B start concurrently.
4. Worker A dispatches first (acquires lock), picks one issue, routes to `implement` agent.
5. Worker B dispatches second, picks the other issue, routes to `implement` agent.
6. Both implement their changes, land them.
7. Both dispatch again, see no remaining issues, output `sleep`.
8. Both exit via `main_head_sha` trigger.

**Snapshot:** concatenated output from init + both workers, showing the full dispatch→implement→land→sleep cycle.

### 4. Trigger handling for multi-step tests

The `[[messages]]` section applies to worker steps (same as today). For the concurrent worker test, the exit trigger fires on `main_head_sha` for both workers — each worker gets its own trigger controller loaded from the same `[[messages]]`.

Init steps don't use triggers (they use stdin directly). Shell steps don't use triggers.

## Questions

### Should the `[scripts]` use heredocs or separate file contents?

The setup script uses heredocs (`cat > ... << 'ISSUE'`) which is a natural way to create multi-line files in a shell script. Alternative: define the issue file contents in the TOML `[files]` section and have the shell script just `git add && git commit`. But the files need to be created *after* init (which creates the `issues/` directory), so they can't go in `[files]` (which runs before everything).

Option 1: **Heredocs in script** — self-contained, the script creates and commits the files
Option 2: **Deferred files section** — add a `[deferred_files]` TOML section that's written after a specific step

Going with Option 1 (heredocs in script) since it's simpler and more flexible.

Answer:

## Review
