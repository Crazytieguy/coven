Issue: [P0] I want to step up the workflow command testing: have a test with `coven init` + multiple `coven worker` running concurrently, test a real situation
Status: draft

## Approach

Add a multi-step VCR test (`concurrent_workers`) that runs `coven init` followed by two concurrent `coven worker` instances against a shared git repo. Uses the real init templates so any template change forces re-recording and snapshot updates for this test.

### 1. Multi-step test infrastructure

**New `[multi]` section in TestCase** — defines an ordered list of named steps:

```toml
[multi]
steps = [
  { name = "init", command = "init", stdin = "y" },
  { name = "worker_a", command = "worker", concurrent_group = "workers" },
  { name = "worker_b", command = "worker", concurrent_group = "workers" },
]
```

Step types:
- `init` — runs `coven init` with the given `stdin`. Records to `<test>__<step>.vcr`.
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
    pub command: String,         // "init", "worker"
    pub stdin: Option<String>,   // for init
    pub concurrent_group: Option<String>,
}
```

`TestCase` gains `multi: Option<MultiConfig>`.

**Changes to record_vcr.rs:**

Add `record_multi_case` alongside the existing `record_case`. Flow:

1. Create temp git repo with `[files]` and initial commit (reuse `setup_test_dir`).
2. Walk steps in order, grouping consecutive steps with the same `concurrent_group`.
3. Sequential steps: run inline, writing VCR files as `<test>__<step>.vcr`.
4. Concurrent group: `tokio::task::spawn_local` each step, join all.
5. Each VCR step captures output to its own buffer.

**Changes to vcr_test.rs:**

Add `multi_vcr_test!` macro (or extend `vcr_test!`). Replay flow:

1. Load all VCR files for the test (`<test>__<step>.vcr` per VCR step).
2. Run steps in the same sequence/concurrency structure, each replaying from its own `VcrContext`.
3. Concatenate outputs with headers (`--- init ---\n`, `--- worker_a ---\n`, etc.).
4. Snapshot the concatenated output.

During replay, each worker replays its own VCR tape independently. Concurrency paths (dispatch lock, state file reads) return pre-recorded values. This is a regression test — the real concurrency validation happens at recording time.

### 2. Test scenario: `concurrent_workers`

**TOML:**

```toml
[multi]
steps = [
  { name = "init", command = "init", stdin = "y" },
  { name = "worker_a", command = "worker", concurrent_group = "workers" },
  { name = "worker_b", command = "worker", concurrent_group = "workers" },
]

[files]
".claude/settings.json" = '{"permissions":{"allow":["Bash(git:*)","Bash(ls:*)","Bash(cat:*)"]}}'
"README.md" = "Helo, world!\n"
"src/main.py" = "print(\"hello\")\n"
"issues/fix-typo.md" = """
---
priority: P1
state: approved
---

# Fix README typo

## Plan

Change "Helo" to "Hello" in README.md. No tests or linter needed — text-only change.
"""
"issues/add-contributing.md" = """
---
priority: P1
state: approved
---

# Create CONTRIBUTING.md

## Plan

Create a CONTRIBUTING.md file with a brief "how to contribute" section. No tests or linter needed.
"""

[[messages]]
content = ""
label = "main_head_sha"
mode = "exit"
```

**Expected flow:**
1. `init` creates `.coven/agents/` (dispatch, plan, implement, audit), `review/`, `.coven/workflow.md`, `CLAUDE.md`. The `issues/` directory already exists with two approved issues.
2. Workers A and B start concurrently.
3. Worker A dispatches first (acquires lock), picks one issue, routes to `implement` agent.
4. Worker B dispatches second, picks the other issue, routes to `implement` agent.
5. Both implement their changes, land them.
6. Both dispatch again, see no remaining issues, output `sleep`.
7. Both exit via `main_head_sha` trigger.

**Snapshot:** concatenated output from init + both workers, showing the full dispatch→implement→land→sleep cycle.

### 3. Trigger handling for multi-step tests

The `[[messages]]` section applies to worker steps (same as today). For the concurrent worker test, the exit trigger fires on `main_head_sha` for both workers — each worker gets its own trigger controller loaded from the same `[[messages]]`.

Init steps don't use triggers (they use stdin directly).

## Questions

## Review
