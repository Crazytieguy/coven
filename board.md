# Blocked

# Ready

## P1: Investigate: some claude sessions don't get displayed by coven

Coven hangs and doesn't display, but claude is actually running in the background. Not a session-exit issue — the process is alive.

**Decisions:**
- Sessions don't exit — coven hangs and doesn't display, but claude is actually running in the background. The original stderr hypothesis was wrong for this issue.
- The stderr fix (capturing stderr instead of null'ing it) is good but is a separate issue — already done.

**New direction:** The problem is that coven's display layer stops showing output even though the claude process is still running. Need to investigate the streaming/rendering pipeline for cases where events are received but not displayed, or where stdout reading stalls.

**Code audit findings (spawn → reader → event loop):**
- Channel replacement (`replace_event_channel`) is clean — creates a fresh (tx, rx) pair, old rx is dropped, new tx goes to reader task. No way to "listen to the wrong session."
- Reader task (`spawn_reader`) correctly takes ownership of stdout via `tokio::spawn`. The `BufReader::lines()` reader handles partial JSON, empty lines, and parse errors without hanging.
- No reference aliasing between old/new channels — Rust's borrow checker prevents `next_event` and `replace_event_channel` from overlapping.
- Fork children use independent channels, never touch `io.event_rx`.
- Initial prompt write to stdin happens before reader task spawn, but can't deadlock — stdin write is async and the prompt is small.
- Minor: `spawn_reader`'s `while let Ok(Some(line))` silently exits on I/O errors — no warning, just `ProcessExit(None)`. Worth fixing but wouldn't cause a hang.

Previous investigation ruled out: event channel replacement, serde fallback, tokio::select fairness, --verbose flag, renderer suppression.

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
