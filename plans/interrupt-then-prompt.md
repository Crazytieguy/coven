Issue: Probably want a way to interrupt a session before prompting (requires restarting it with --resume, same as if the session ends organically)
Status: draft

## Approach

Currently Ctrl+C during a running session kills the Claude process and immediately exits the program (both in run mode and ralph mode). The desired behavior is: Ctrl+C stops the current response but then shows the follow-up prompt, so the user can either send a new message (resuming the session) or exit cleanly.

This is the "polite interrupt" — stop what Claude is doing, but stay in the conversation.

### Core change

Add a new `SessionOutcome::Interrupted` behavior that flows into follow-up prompting instead of exiting.

#### `src/commands/session_loop.rs`

The Ctrl+C handler (currently lines ~136-138) kills the runner and returns `SessionOutcome::Interrupted`. No change needed here — the outcome already exists and the kill behavior is correct.

#### `src/commands/run.rs`

Currently `SessionOutcome::Interrupted` breaks the loop. Change it to:
1. Show the follow-up prompt (same as `Completed` path)
2. If the user sends a follow-up, restart with `--resume <session_id>` and the new message as the prompt
3. If the user presses Ctrl+C again or Ctrl+D at the prompt, exit

This requires the session_id to be available after interruption. `SessionState.session_id` is set when the Init event is received, so it should be populated by the time the user interrupts (unless they interrupt before Init, which is an edge case — just exit in that case).

The follow-up after interrupt differs from follow-up after completion in one way: we need `--resume` because the session was killed mid-stream. After a normal completion, the session is still alive and follow-ups go through stdin. After an interrupt, the process is dead so we must start a new process with `--resume`.

Concretely, this means `wait_for_followup` stays the same, but the caller (run.rs main loop) needs to handle the resumed session:
- Create a new `SessionConfig` with `resume: Some(session_id)` and `prompt: Some(followup_text)`
- Spawn a new runner
- Continue the loop

This requires adding `resume: Option<String>` to `SessionConfig` and handling it in `build_args()` — which is the same change proposed in the steering redesign plan. Since steering redesign is still draft, this plan can implement the `--resume` plumbing independently; steering will reuse it.

#### `src/commands/ralph.rs`

Same pattern: `Interrupted` should show the follow-up prompt instead of breaking the ralph loop. If the user sends a message, resume within the same iteration. If they exit the prompt, break the ralph loop as today.

#### `src/session/runner.rs`

- Add `resume: Option<String>` field to `SessionConfig`
- In `build_args()`, when `resume` is `Some(id)`, replace `-p` with `--resume <id>` (resumed sessions use `--resume` instead of `-p`)

### Edge cases

- **Interrupt before Init event**: `session_id` is None. Just exit as today — there's nothing to resume.
- **Multiple interrupts**: User interrupts, gets prompt, sends message, interrupts again. Each cycle kills the process and shows a new prompt. Works naturally with the loop structure.
- **Ctrl+D at follow-up prompt after interrupt**: Clean exit, same as today's Ctrl+D behavior.

## Questions

### Should Ctrl+C show a visual indicator before the prompt?

When the user presses Ctrl+C mid-stream, the partial output stops abruptly. Options:

- **Just show the prompt**: The `> ` prompt appearing is sufficient indication that the session was interrupted. Simplest.
- **Show `[interrupted]` then prompt**: Explicit indicator like `[interrupted]` on a new line before showing the prompt. More informative.

Answer:

### Should this work identically in run mode and ralph mode?

In run mode, interrupting and resuming is straightforward — you stay in the same conversation. In ralph mode, the session is part of an iteration loop. Options:

- **Same behavior**: Interrupt shows prompt, follow-up resumes within the current iteration. Exiting the prompt ends the ralph loop entirely.
- **Ralph-specific**: Interrupt shows prompt, but exiting the prompt moves to the next iteration instead of ending the loop. This gives the user a way to skip a stuck iteration.

Answer:

## Review

