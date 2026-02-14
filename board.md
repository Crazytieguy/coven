# Board

## P1: Dynamic terminal title for coven worker

Research approaches for letting `coven worker` set the terminal title dynamically based on the current agent/task. Think of a few approaches and document trade-offs. This is a design task — no implementation yet.

### Current behavior

Title is set at 4 hardcoded points in `worker.rs`:
- Start: `cv {branch}`
- Per agent: `cv {agent} {key=val ...} — {branch}` (e.g. `cv main task=Investigate compaction — nimble-eagle-49`)
- Sleep: `cv sleeping — {branch}`
- Exit: cleared

The per-agent title is useful but verbose. The `key=val` format is ugly, and there's no way for agents to update the title mid-session.

### Approaches

**A. Title template in agent frontmatter**

Add optional `title` field to `AgentFrontmatter` — a Handlebars template rendered with the same args map used for the prompt. Falls back to current format if absent.

```yaml
# .coven/agents/main.md
description: "Implements work"
title: "{{task}}"
args:
  - name: task
    required: true
```

Result: `cv Fix scroll bug — nimble-eagle-49`

- Pro: Declarative, zero runtime cost, per-agent control
- Pro: Handlebars already available (used for prompt rendering)
- Pro: Trivial implementation — ~10 lines in `worker.rs` + 1 field in `AgentFrontmatter`
- Con: Static per invocation — title can't change mid-session
- Con: Agents without `title` field keep the raw `key=val` format

**B. `<title>` tag in agent output (mid-session updates)**

New tag that agents output during execution. The session loop parses it and calls `set_title()`.

```
<title>building feature</title>
...
<title>running tests</title>
```

- Pro: Most dynamic — live progress in terminal title
- Pro: Agent knows best what phase it's in
- Con: Requires protocol parsing changes in session loop
- Con: Models need prompting to emit it; unreliable with smaller models
- Con: Could be noisy; title flickers if emitted frequently
- Con: More complex — touches protocol layer, not just worker

**C. Convention: derive title from well-known arg**

Hardcode a convention: if the agent has a `task` arg, use its value as the title. No config needed.

`cv main task=Fix scroll bug — branch` → `cv main: Fix scroll bug — branch`

- Pro: Zero configuration, works today
- Pro: Matches current usage (dispatch always passes `task`)
- Con: Inflexible — only works for `task` arg
- Con: Breaks if agents use different arg names for the primary label
- Con: Doesn't help dispatch or custom agents

**Decisions:**
- Use approach A (title template in agent frontmatter)
- Keep the branch name in the title

---

## P1: Investigate compaction handling

Test whether coven correctly handles auto-compaction during long sessions. Trigger compaction by having haiku repeatedly read files with output each time. Document the observed behavior and any issues found. Do not implement a fix.

## P1: Improve agent prompt conciseness and stale content cleanup

Two prompt improvements:
1. Make the main agent more concise when asking questions (currently too verbose)
2. Have the dispatch agent remove stale content from board issues (old design notes, resolved alternatives, etc.) when syncing
