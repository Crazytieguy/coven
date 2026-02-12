---
priority: P1
state: new
---

# Per-agent concurrency semaphores

## Motivation

Split from the generic agent loop issue. The current system uses a single exclusive dispatch lock. The generic agent loop replaces this with per-agent concurrency control, but that's a clean add-on that can be done after the core loop refactor lands.

## Design

### `max_concurrency` frontmatter field

Agent frontmatter gains an optional `max_concurrency` field:

```yaml
---
description: "Route work to agents based on issue state"
max_concurrency: 1
args: ...
---
```

Default if unspecified: unlimited (no concurrency restriction).

### Counted file lock semaphores

Before running an agent, the worker acquires a semaphore permit for that agent type. Implementation: counted file locks in `<git-common-dir>/coven/semaphores/`.

For an agent with `max_concurrency: N`:
- Semaphore files: `<git-common-dir>/coven/semaphores/<agent>.0.lock` through `<agent>.<N-1>.lock`
- Worker tries `try_lock_exclusive` on each file in sequence
- If all N are locked, async-retry with sleep (like current dispatch lock)
- Lock released on drop (RAII)

### Changes

1. Add `max_concurrency: Option<u32>` to `AgentFrontmatter` in `src/agents.rs`
2. New module `src/semaphore.rs` implementing counted file lock semaphores
3. In `worker_loop`: before running any agent, acquire semaphore if `max_concurrency` is set
4. Remove the temporary dispatch lock (from the generic loop issue) — replace with semaphore acquisition for the entry agent
5. Update dispatch agent template: add `max_concurrency: 1`
6. Update `coven init` templates accordingly

### Depends on

- Generic agent loop (issues/worker-generic-agent-loop.md)
