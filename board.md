# Blocked

## P1: Investigate: some claude sessions don't get displayed by coven

Deep code audit of the full pipeline. No architectural bugs found — data flow is sound.

**What's been ruled out (cumulative):**
- Event channel replacement, serde fallback, tokio::select fairness, --verbose flag, renderer suppression
- Channel aliasing, fork interference, stdin deadlock, reader I/O hang, renderer blocking
- Reader task panic (would cause ProcessExit, not a hang)
- Race between channel replacement and event loop (borrow checker prevents it)
- Premature session termination as cause of invisible-but-worker-alive (any non-Completed outcome exits the worker — no recovery path)

**New finding: tool counter evidence**

The renderer's `tool_counter` persists across all sessions in the worker (created once, never reset, only increments). In the reproduction case: main agent's last tool was [310], displayed review session starts at [311]. Any session going through `run_session` that receives tool calls, thinking blocks, or compaction events would increment the counter. The invisible session reportedly did extensive work (file reads, git commands, landing). The absence of a counter gap is strong evidence the invisible session **did not go through the renderer**.

**The contradiction:**

Four facts that can't all be true:
1. The invisible session completed work (issue moved to Done, scratch.md deleted, changes landed)
2. The invisible session didn't go through the renderer (tool counter evidence)
3. All code paths through `run_session` render events — there's no bypass
4. The worker stayed alive after the invisible session (the displayed review ran)

If #2 is true, the session didn't go through `run_session`. But #3 says all sessions go through `run_session`. If a session failed before rendering, the worker would exit (#4 contradicts).

**Remaining hypotheses (one of the four "facts" must be wrong):**

1. **Terminal rendering failure (fact #2 is wrong):** Output was produced by coven, but the terminal emulator didn't display it. The tool counter DID increment; the [311] we see is actually [311] only because the terminal swallowed the invisible session's output. The user's scrollback might contain the missing output. This is unfalsifiable without terminal logs.

2. **External execution (fact #3 is wrong):** The invisible session ran outside of coven — e.g., a separate `claude` invocation from another terminal, or the main agent itself doing review work earlier in its 310 tool calls (we only see the last 4). This seems unlikely but can't be ruled out.

3. **Evidence misinterpretation (fact #1 is wrong):** The work wasn't done by an invisible session. Possible alternatives: the main agent ran `land.sh` during its 310 tool calls as a test; another worker's changes coincidentally resolved the issue; or the state was altered by manual intervention.

**Questions:**
- When this happened, did you check the terminal scrollback? If the missing output is there (even partially), that points to a terminal rendering issue rather than a coven bug.
- Do you have the full output from the main agent's session (all 310 tool calls)? If the main agent ran `land.sh` at any point, that would explain the state the review agent found.
- Is it possible another terminal or process was interacting with the worktree?

# Ready

# Done

- P1: Coordinate worker sleep — if one dispatch sleeps, others should too

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
