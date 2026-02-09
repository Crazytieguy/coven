Issue: [P0] VCR + snapshot testing for concurrent worker sessions. Needs design — recording multiple interleaved sessions, snapshot format for parallel output, and how to make tests deterministic.
Status: draft

## Approach

### Problem

The worker loop (`commands/worker.rs`) involves multiple sequential claude sessions per worker (dispatch, then agent), and multiple workers run concurrently. The current VCR system records and replays a single session. We need to extend it to support testing the full worker lifecycle with multiple concurrent workers.

### What to test

The worker test should exercise:
1. The dispatch → agent → land loop for each worker
2. Multiple workers running concurrently with dispatch lock coordination
3. The rendered terminal output across parallel workers

### Recording format

Each worker run involves multiple distinct claude sessions (dispatch session, then agent session, possibly conflict resolution session). Propose recording each session as a separate VCR file and using a test manifest to compose them.

**Multi-worker test case format** (new `.toml` structure):

```toml
[worker_test]
num_workers = 2

# Each worker gets a sequence of sessions
[[worker_test.workers]]
name = "worker-a"
sessions = [
    { role = "dispatch", vcr = "worker_basic_dispatch_a" },
    { role = "agent", vcr = "worker_basic_agent_a" },
]

[[worker_test.workers]]
name = "worker-b"
sessions = [
    { role = "dispatch", vcr = "worker_basic_dispatch_b" },
    { role = "agent", vcr = "worker_basic_agent_b" },
]
```

Each referenced VCR file is a standard single-session recording (reusing the existing format). This avoids inventing a new multi-stream VCR format.

### Deterministic interleaving

Real concurrent workers have non-deterministic timing. For tests, propose an **event-driven interleaving** approach:

1. Replace real tokio concurrency with a deterministic scheduler
2. Define interleave points in the test manifest (e.g., "worker-a completes dispatch, then worker-b starts dispatch")
3. Alternatively, run workers sequentially in tests but verify they could have run concurrently (simpler, tests less)

The simplest viable approach: **run workers sequentially in a defined order** and snapshot each worker's output independently. This tests the dispatch → agent → land pipeline and worker state coordination without requiring a concurrent scheduler. True concurrent interleaving tests can be added later.

### Git operation mocking

Worker tests need git operations (worktree creation, branching, rebase, merge). Options:

- **Real git**: Create a real repo in a temp dir, let workers operate on it. Pros: tests the actual git integration. Cons: slower, more setup.
- **Mocked git**: Abstract git operations behind a trait, provide a test impl. Pros: faster, more deterministic. Cons: doesn't test real git behavior, significant refactoring.

Recommend **real git in temp dirs** since the git integration is critical to correctness and the existing VCR tests already use temp dirs with real git.

### Snapshot format

For sequential worker execution, each worker gets its own snapshot (like existing single-session snapshots). The snapshot includes:
- Dispatch phase output (which agent was selected)
- Agent phase output (tool calls, text)
- Land phase output (rebase/merge result)

For future concurrent rendering (separate P2 issue), a combined snapshot format would be needed, but that's out of scope here.

### Session injection

The worker currently spawns `SessionRunner` directly, calling `claude -p`. For tests, we need to inject VCR replay instead. Approach:

1. Abstract session creation behind a trait or callback
2. In production: spawns real claude process
3. In tests: returns a replay session that feeds VCR events

This is the main code change — the worker needs a seam for session injection.

### Implementation steps

1. Define `SessionFactory` trait (or equivalent) that `worker.rs` uses instead of directly constructing `SessionRunner`
2. Implement `VcrSessionFactory` for tests that serves pre-recorded sessions in order
3. Create the multi-worker test case format (extend `vcr.rs` TestCase enum)
4. Write test infrastructure: temp repo setup, worker state initialization, sequential worker execution
5. Record a basic 2-worker test case (two dispatch + two agent sessions)
6. Add snapshot testing for worker output
7. Add a test for dispatch lock coordination (worker-b sees worker-a's state)

## Questions

### Sequential vs concurrent test execution

Should worker tests run workers sequentially (simpler, deterministic) or attempt concurrent execution with a deterministic scheduler (more realistic, much more complex)?

Sequential testing still validates the core loop, session injection, state coordination, and landing. Concurrent execution testing would additionally validate timing-dependent behavior.

Answer:

### Session injection approach

Should we use a trait-based approach (`SessionFactory` trait) or a simpler approach like passing a closure/callback for session creation? The trait approach is more idiomatic Rust but adds a generic parameter. The closure approach is simpler but less type-safe.

Answer:

### Scope of first test case

What scenario should the first worker test cover? Options:
- **Minimal**: Single worker, one dispatch + one agent, no conflicts (proves the pipeline works)
- **Basic concurrent**: Two workers, sequential execution, worker-b's dispatch sees worker-a's state
- **Conflict resolution**: Two workers that produce conflicting changes, testing the rebase/resolve path

Answer:

## Review

