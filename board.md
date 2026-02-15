# Blocked

# Ready

## P1: Investigate: some claude sessions don't get displayed by coven

Deep audit of the full pipeline (spawn → reader → channel → event loop → renderer). No architectural bugs found — the data flow is sound.

**What's been ruled out (cumulative):**
- Event channel replacement, serde fallback, tokio::select fairness, --verbose flag, renderer suppression (previous sessions)
- Channel aliasing, fork interference, stdin deadlock, reader I/O hang, renderer blocking (this session)
- Reader task panic (would cause ProcessExit, not a hang)
- Race between channel replacement and event loop (borrow checker prevents it)

**Two concrete observability gaps found:**

1. **Stderr is batched, not streamed.** `spawn_reader` collects ALL stderr via `read_to_string` in a background task, only surfacing it after stdout closes. If the claude CLI outputs startup diagnostics to stderr (auth, rate limits, loading, MCP), coven shows nothing until the session ends. If the CLI hangs during startup, coven is completely silent — no way to tell what's happening.

2. **No reader heartbeat.** When the reader task is blocked on `next_line()` waiting for the first stdout line, there's no timeout or diagnostic. Can't distinguish "claude is slow to start" from "reader is stuck" from "claude hung during auth."

**One potential hang source:**

`runner.wait()` in the worker's `run_phase_session` (line 741) has no timeout. If claude doesn't exit promptly after stdin close (e.g., mid-tool-use), this blocks indefinitely. The process is alive (matches the symptom), but coven isn't reading its output anymore (event loop already returned).

**New evidence (concrete reproduction case):**

Human observed a worker transition main → review where the review agent ran twice: the first run completed invisibly (landed changes, moved issue to Done, cleaned scratch.md), but coven displayed nothing during it — appeared to hang. The second review run was displayed normally but found nothing to do (issue already Done, no commits ahead of main, scratch.md missing). The displayed review session's behavior only makes sense if a prior invisible session already completed the work.

**Decisions:**
- Don't proceed with stderr streaming + heartbeat yet — human says proposed root causes may not be correct. Use the new reproduction evidence to investigate further first.
- The invisible-session evidence suggests the issue may be specific to worker agent transitions, not a general startup/reader problem.

# Done

- P1: Review: is `git reset --hard main` correct in the review agent?
- P1: Implement new board format (replace divider with Blocked/Ready sections)
- P2: Capture stderr from claude process
- P1: Split main into main + review agents
- P1: First typed character after entering interactive with Ctrl+O seems to be swallowed
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
- P1: wait-for-user re-proposal
- P1: Simplify status line after exiting embedded interactive session
- P1: wait-for-user prompt final revision
- P2: scratch.md: should clarify that it's gitignored
