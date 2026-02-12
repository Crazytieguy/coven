---
priority: P1
state: approved with comments
---

# Let coven agents define `claude` arguments

Agents should be able to declare extra arguments to pass to the `claude` CLI. This could be a frontmatter field in the agent template (e.g. `claude_args`).

Use this in the default agent templates to grant permissions each agent needs — for example, commonly used git operations and the `land.sh` script.

## Plan

### 1. Add `claude_args` field to `AgentFrontmatter` (`src/agents.rs`)

Add an optional `claude_args` field to the `AgentFrontmatter` struct:

```rust
#[serde(default)]
pub claude_args: Vec<String>,
```

This is a flat list of strings, matching how `extra_args` already flows through the system (e.g. `["--allowedTools", "Bash(git *)"]`).

Add a unit test for parsing an agent with `claude_args` in frontmatter.

### 2. Merge per-agent args in `run_agent_chain` (`src/commands/worker.rs`)

In `run_agent_chain`, after resolving the agent definition (line ~302), merge the agent's `claude_args` with the worker-level `config.extra_args`. Agent args come first, worker args last (so CLI-level `-- [ARGS]` can override agent defaults):

```rust
let mut merged_args = agent_def.frontmatter.claude_args.clone();
merged_args.extend(config.extra_args.iter().cloned());
```

Pass `&merged_args` instead of `&config.extra_args` to `run_phase_session` (line ~331) and to `parse_transition_with_retry` (line ~352).

### 3. Update default agent templates (`.coven/agents/*.md`)

Add `claude_args` to each agent's frontmatter with the permissions it needs:

**dispatch.md** — Needs `head` on issue files:
```yaml
claude_args:
  - "--allowedTools"
  - "Bash(head *)"
```

**plan.md** — Needs git operations, file exploration, and landing:
```yaml
claude_args:
  - "--allowedTools"
  - "Bash(git add:*)"
  - "--allowedTools"
  - "Bash(git mv:*)"
  - "--allowedTools"
  - "Bash(git commit:*)"
  - "--allowedTools"
  - "Bash(git log:*)"
  - "--allowedTools"
  - "Bash(git diff:*)"
  - "--allowedTools"
  - "Bash(git status)"
  - "--allowedTools"
  - "Bash(bash .coven/land.sh)"
  - "--allowedTools"
  - "Bash(cargo:*)"
```

Review note: a comma separated list of tools is valid. Cargo should **not** be in the default template, as it's specific to this project. Users are expected to set up anything they need beyond these tools. Needs rebase. Read only git tools: check if these are automatically approved by claude code before adding them (the vcr tests will tell us)

**implement.md** — Needs git operations, build/test/lint tools, and landing:
```yaml
claude_args:
  - "--allowedTools"
  - "Bash(git add:*)"
  - "--allowedTools"
  - "Bash(git mv:*)"
  - "--allowedTools"
  - "Bash(git rm:*)"
  - "--allowedTools"
  - "Bash(git commit:*)"
  - "--allowedTools"
  - "Bash(git log:*)"
  - "--allowedTools"
  - "Bash(git diff:*)"
  - "--allowedTools"
  - "Bash(git status)"
  - "--allowedTools"
  - "Bash(cargo:*)"
```

Review note: same (doesn't need rebase though)

**land.md** — Needs git operations and the land script:
```yaml
claude_args:
  - "--allowedTools"
  - "Bash(git log:*)"
  - "--allowedTools"
  - "Bash(git diff:*)"
  - "--allowedTools"
  - "Bash(git status)"
  - "--allowedTools"
  - "Bash(git add:*)"
  - "--allowedTools"
  - "Bash(git commit:*)"
  - "--allowedTools"
  - "Bash(git rebase:*)"
  - "--allowedTools"
  - "Bash(bash .coven/land.sh)"
  - "--allowedTools"
  - "Bash(cargo:*)"
```

Review note: same

### 4. Tests

- Add a unit test in `agents.rs` that parses an agent with `claude_args` and verifies the field is populated.
- Add a unit test that `claude_args` defaults to empty when omitted (existing tests already cover this implicitly via `#[serde(default)]`).

## Questions

- The `--allowedTools` flag syntax for `claude` CLI — does it accept the `Bash(pattern)` format shown above, or is there a different syntax? I've assumed `--allowedTools "Bash(git add:*)"` based on the Claude Code permission model, but need to confirm the exact CLI flag and pattern format.
Answer: answered above
- Should all agents also get `--allowedTools "Bash(head *)"` since they may need to inspect issue files, or is that only needed for dispatch?
Answer: just dispatch
- Are there other permissions the agents commonly need that should be included (e.g. `rm` for deleting issue files in implement)?
Answer: not critical for now

## Review

We're going to need to remove the redundant permissions from the relevant vcr tests, and rerecord. Iteration might be needed: make sure to look at the rejected tool calls in the vcr recording or snapshot output and add any necessary permissions. Note that agents might long for a long time if they're lacking necessary permissions, so you should make sure to stop the recording if this happens (run in background and watch)
