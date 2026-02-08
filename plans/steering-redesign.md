Issue: Confirmed: `claude -p --input-format stream-json` ignores stdin messages sent mid-stream. Steering needs to be redesigned — likely by interrupting the session and resuming with the steering message as a follow-up.
Status: rejected

## Approach

Since Claude's `--input-format stream-json` ignores messages written to stdin while it's streaming a response, steering must be implemented by **interrupting the current process and resuming the session** with the steering message as a new turn.

### Core mechanism

When a steering message is submitted (Enter while Claude is streaming):

1. **Kill** the current Claude process (`runner.kill()`)
2. **Wait** for it to exit (so stdout closes and the reader task ends)
3. **Spawn** a new Claude process with `--resume <session_id>` and the steering message as the prompt
4. **Continue** the event loop with the new runner

The Claude backend preserves conversation state server-side, so `--resume` picks up from the last completed assistant message. The partial response being generated when we killed the process is discarded — which is exactly what the user wants when steering.

### Changes by file

#### `src/session/runner.rs`

- Add `resume: Option<String>` field to `SessionConfig`
- In `build_args()`, when `resume` is `Some(id)`, add `--resume <id>` to args

#### `src/commands/session_loop.rs`

- Add `Steered { message: String }` variant to `SessionOutcome`
- In `handle_session_key_event`, when `InputMode::Steering`:
  - Instead of `runner.send_message()`, call `runner.kill().await`
  - Return a new `LoopAction::Steered(text)` which maps to `SessionOutcome::Steered { message }`
- Drain any remaining events from `event_rx` after kill (to avoid stale events from the dead process)

#### `src/commands/run.rs`

- Keep `event_tx` alive across session restarts (clone instead of move when spawning)
- Handle `SessionOutcome::Steered { message }` in the main loop:
  - Wait for old runner to exit
  - Create new `(event_tx, event_rx)` channel pair
  - Build `SessionConfig` with `resume: Some(session_id)` and `prompt: Some(message)`
  - Spawn new runner, reset `SessionState`, continue loop
- Need to store `extra_args` for reuse (currently moved into config)

#### `src/commands/ralph.rs`

- Handle `SessionOutcome::Steered { message }` — same pattern as run.rs but within the iteration loop. Restart with `--resume` within the same iteration rather than moving to the next one.

#### Display behavior

When steering triggers a restart:

- Current partial output stays on screen (it's already rendered)
- New session's init event renders a new session header below
- Claude's response to the steering message renders normally

No special renderer changes needed — the existing flow handles this naturally.

### Related issue: steering VCR test

The existing `steering.vcr` test is a false positive (validates stdin write, not Claude's response). After this redesign:

- The steering test should be **removed or rewritten** — the current VCR infrastructure records a single process, but steering now involves killing and restarting a process
- VCR testing of steering would require multi-process recording support, which is a separate effort
- For now, remove the steering test case and add an issue to design multi-process VCR testing if needed

### What this does NOT change

- **Follow-up messages** (Alt+Enter) — still work the same way (buffered until result, then sent via stdin)
- **Event buffering during input** — still works (events buffered while typing, flushed on submit/cancel)
- **Ralph mode loop** — iterations still restart fresh sessions; steering just adds a mid-iteration restart

## Questions

### Should we show a visual indicator when steering triggers a session restart?

When the user submits a steering message, there's a brief gap while the old process is killed and the new one starts. Options:

- **No indicator**: The new session header (from the init event) serves as implicit indication. Simplest approach.
- **Explicit indicator**: Print something like `[redirecting...]` between the old output and new session header. More informative, especially if the restart takes a moment.

Answer:

### How should we handle the steering VCR test?

The current steering test (`tests/cases/steering.*`) is a false positive. With the redesign, VCR testing of steering requires multi-process recording which doesn't exist yet. Options:

- **Remove the test entirely** and track multi-process VCR testing as a separate issue
- **Convert to a simpler test** that only validates the interrupt+resume mechanism without full VCR (e.g., a unit test that checks `SessionConfig` builds the right args with `--resume`)
- **Keep the test but mark it as known-broken** (not recommended — false positives are worse than no test)

Answer:

### Should `--resume` carry over `--append-system-prompt`?

In ralph mode, sessions use `--append-system-prompt` for the ralph system prompt. When steering causes a restart with `--resume`, should the resumed session also include the system prompt append? Claude may already have it in the conversation context from the original session, so appending it again could be redundant or cause duplication.

Answer:

## Review

This whole plan is based on a false premise, I've seen the steering prompts work with my own eyes. If we don't have a good vcr + snapshot test that demonstrates this, we should add one (long task with a steering prompt). Remove the original issue.
