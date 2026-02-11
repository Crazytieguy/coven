---
priority: P1
state: review
---

# Design VCR tests that evaluate orchestration quality

The current orchestration tests (`worker_basic`, `concurrent_workers`) cover basic functionality. We need tests for harder scenarios that evaluate how well models pilot the orchestration system — e.g. correct dispatch decisions under competing priorities, proper state transitions when implementation fails, conflict resolution during landing, planning quality for ambiguous issues.

These tests double as evals: the VCR snapshots capture model behavior given the prompts and systems, so improving prompts/agents can be validated by re-recording and checking snapshot diffs.

Come up with several unique test scenarios that stress-test orchestration decision-making.

## Plan

Four new multi-step VCR eval tests under `tests/cases/orchestration/`, all using real agents via `coven init`. Each test targets a specific orchestration decision-making scenario not covered by existing tests.

### Test 1: `plan_ambiguous_issue`

**Evaluates**: Planning quality for vague requirements, correct `new → review` lifecycle.

**Flow**: dispatch → plan agent → commit (file moved to review/) → dispatch → sleep

**TOML** (`tests/cases/orchestration/plan_ambiguous_issue/plan_ambiguous_issue.toml`):
```toml
[[multi.steps]]
name = "init"
command = "init"
stdin = "y"

[[multi.steps]]
name = "worker"
command = "worker"

[[multi.steps.messages]]
content = ""
label = "main_head_sha"
mode = "exit"

[files]
".claude/settings.json" = '{"permissions":{"allow":["Bash(git:*)","Bash(ls:*)","Bash(cat:*)","Bash(mkdir:*)"]}}'
"src/app.py" = """import sys

def main():
    data = sys.stdin.read()
    result = process(data)
    print(result)

def process(data):
    return data.upper()

if __name__ == "__main__":
    main()
"""
"tests/test_app.py" = """from src.app import process

def test_process():
    assert process("hello") == "HELLO"
"""
"issues/improve-error-handling.md" = """---
priority: P1
state: new
---

# Improve error handling

The error handling in the project needs improvement. Users sometimes see unhelpful error messages when things go wrong.
"""
```

**Eval signals in snapshot**:
- Does dispatch correctly route `new` state to the `plan` agent?
- Does the plan agent explore the codebase before writing the plan?
- Does the plan contain a `## Questions` section surfacing ambiguities (e.g., which errors? what should "improvement" look like?)
- Is the plan specific enough to implement despite vague requirements?
- Is the file moved to `review/` with `state: review`?
- Does dispatch sleep after (only item is in review)?

### Test 2: `priority_dispatch`

**Evaluates**: Correct dispatch ordering under competing issue priorities.

**Flow**: dispatch → implement P0 → land → dispatch → implement P1 → land → dispatch → sleep

**TOML** (`tests/cases/orchestration/priority_dispatch/priority_dispatch.toml`):
```toml
[[multi.steps]]
name = "init"
command = "init"
stdin = "y"

[[multi.steps]]
name = "worker"
command = "worker"

[[multi.steps.messages]]
content = ""
label = "main_head_sha"
mode = "exit"

[files]
".claude/settings.json" = '{"permissions":{"allow":["Bash(git:*)","Bash(ls:*)","Bash(cat:*)","Bash(mkdir:*)"]}}'
"README.md" = "# My Project\n\nA sample project.\n"
"src/main.py" = "print(\"hello\")\n"
"issues/fix-readme.md" = """---
priority: P0
state: approved
---

# Fix README description

## Plan

Change "A sample project." to "A sample project for demonstration." in README.md.
"""
"issues/improve-main.md" = """---
priority: P1
state: approved
---

# Improve main.py

## Plan

Wrap the print statement in src/main.py in a `if __name__ == "__main__":` guard.
"""
```

**Eval signals in snapshot**:
- Does dispatch examine all issues and reason about priorities?
- Does it pick P0 (`fix-readme`) before P1 (`improve-main`)?
- Does it explain the priority reasoning?
- After P0 completes, does it correctly move to P1?
- After both complete, does it sleep?

### Test 3: `needs_replan`

**Evaluates**: Implementation failure → `needs-replan` state transition → replanning cycle.

**Flow**: dispatch → implement (fails, references nonexistent code) → commit needs-replan → dispatch → plan (revises based on failure notes) → commit → dispatch → sleep

**TOML** (`tests/cases/orchestration/needs_replan/needs_replan.toml`):
```toml
[[multi.steps]]
name = "init"
command = "init"
stdin = "y"

[[multi.steps]]
name = "worker"
command = "worker"

[[multi.steps.messages]]
content = ""
label = "main_head_sha"
mode = "exit"

[files]
".claude/settings.json" = '{"permissions":{"allow":["Bash(git:*)","Bash(ls:*)","Bash(cat:*)","Bash(mkdir:*)"]}}'
"src/app.py" = """def greet(name):
    return f"Hello, {name}!"

if __name__ == "__main__":
    print(greet("World"))
"""
"issues/add-validation.md" = """---
priority: P1
state: approved
---

# Add input validation to processor

## Plan

1. Open `src/processor.py` and add validation to the `process_data` function
2. The validation should check that the input is not empty and is a valid string
3. Add a `ValidationError` exception class at the top of the file
4. Update the existing tests in `tests/test_processor.py`
"""
```

The plan references `src/processor.py` and `tests/test_processor.py`, neither of which exist. The implement agent should recognize the plan is broken.

**Eval signals in snapshot**:
- Does the implement agent detect that the plan references nonexistent files?
- Does it set `state: needs-replan` and write an `## Implementation Notes` section explaining the problem?
- Does it avoid committing broken code (only commits the updated issue)?
- Does dispatch route `needs-replan` back to the plan agent?
- Does the plan agent read the implementation notes and revise the plan to match the actual codebase (`src/app.py`)?
- Is the revised plan grounded in files that actually exist?

### Test 4: `landing_conflict`

**Evaluates**: Rebase conflict resolution during concurrent landing.

**Flow**: Both workers implement changes to the same line of the same file; one lands first, the other hits a rebase conflict, resolves it, and lands.

**TOML** (`tests/cases/orchestration/landing_conflict/landing_conflict.toml`):
```toml
[[multi.steps]]
name = "init"
command = "init"
stdin = "y"

[[multi.steps]]
name = "worker_a"
command = "worker"
concurrent_group = "workers"

[[multi.steps.messages]]
content = ""
label = "main_head_sha"
mode = "exit"

[[multi.steps]]
name = "worker_b"
command = "worker"
concurrent_group = "workers"

[[multi.steps.messages]]
content = ""
label = "main_head_sha"
mode = "exit"

[files]
".claude/settings.json" = '{"permissions":{"allow":["Bash(git:*)","Bash(ls:*)","Bash(cat:*)","Bash(mkdir:*)"]}}'
"README.md" = "# My Project\n\nA sample project for testing.\n"
"issues/update-title-v2.md" = """---
priority: P1
state: approved
---

# Update README title to v2

## Plan

Change the first line of README.md from "# My Project" to "# My Project v2". Commit with message "Update project title to v2".
"""
"issues/rename-project.md" = """---
priority: P1
state: approved
---

# Rename project in README

## Plan

Change the first line of README.md from "# My Project" to "# Sample App". Commit with message "Rename project in README".
"""
```

Both issues modify line 1 of README.md, guaranteeing a rebase conflict for whichever worker lands second.

**Eval signals in snapshot**:
- Do both workers select different issues (concurrent dispatch awareness)?
- Does the second-to-land worker hit a `Rebase conflict` message?
- Does the conflict resolution session correctly resolve the merge conflict?
- Does the resolved version land successfully?
- Is the final content coherent (not corrupted merge markers)?

### Implementation Steps

1. Create test directories:
   - `tests/cases/orchestration/plan_ambiguous_issue/`
   - `tests/cases/orchestration/priority_dispatch/`
   - `tests/cases/orchestration/needs_replan/`
   - `tests/cases/orchestration/landing_conflict/`

2. Write the four TOML configs as specified above.

3. Register tests in `tests/vcr_test.rs`:
   ```rust
   multi_vcr_test!(orchestration / plan_ambiguous_issue);
   multi_vcr_test!(orchestration / priority_dispatch);
   multi_vcr_test!(orchestration / needs_replan);
   multi_vcr_test!(orchestration / landing_conflict);
   ```

4. Record fixtures one at a time, reviewing each before moving on:
   ```
   cargo run --bin record-vcr plan_ambiguous_issue
   cargo run --bin record-vcr priority_dispatch
   cargo run --bin record-vcr needs_replan
   cargo run --bin record-vcr landing_conflict
   ```

5. After each recording, review the snapshot for the eval signals listed per test. If the model doesn't produce expected behavior (e.g., implement agent doesn't set needs-replan), adjust the test setup (files, issue description) and re-record.

6. Run `cargo test` to verify all replays succeed.

7. Accept snapshots: `cargo insta accept`.

8. Final check: `cargo fmt && cargo clippy && cargo test`.

### Notes

- All tests use `coven init` as the first step to get the real agent prompts. This means re-recording after prompt changes (e.g., the `fix-plan-vs-implement-priority` or `explicit-review-cap` issues) will show behavior diffs in snapshots.
- Permissions in `.claude/settings.json` may need adjustment during recording if agents need additional shell commands. Start with the listed set and expand if recording fails on permission prompts.
- The `needs_replan` test involves 3 dispatch rounds (dispatch → implement → dispatch → plan → dispatch → sleep), making it the longest test to record. If recording is unreliable, it can be split into two simpler tests.

## Questions

**Should all four tests use haiku (the current default `DEFAULT_TEST_MODEL`) or should any use a more capable model?** Haiku is cheaper/faster but may struggle with the `needs_replan` scenario (correctly detecting the plan is broken and setting needs-replan). The existing orchestration tests all use haiku.
