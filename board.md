# Board

## P1: Investigate: some claude sessions don't get displayed by coven

Maybe listening to the wrong session or something weird like that.

*In progress (prime-cedar-53)*

---

## P1: Investigate: some claude sessions don't get displayed by coven

Investigated the full event pipeline: spawning → stdout reading → parsing → event channel → renderer.

**Finding: stderr is completely suppressed.** `runner.rs:75` sets `.stderr(Stdio::null())`, so if the claude CLI encounters an error (auth failure, API rate limits, invalid args, model unavailable), the error goes to stderr (invisible) and the process exits with no stdout. The user sees only "Claude process exited" with zero context.

This is the most likely cause of "sessions that don't get displayed" — the claude process fails before producing any stream-json output, and the error message is swallowed.

**Other things I checked (less likely):**
- Event channel replacement (`io.replace_event_channel()`) — sequential design prevents lost events
- `InboundEvent` enum has no `#[serde(other)]` fallback, but unknown types show as parse warnings (visible, not silent)
- `tokio::select!` fairness in `next_event()` — can delay events but not lose them
- `--verbose` flag is always passed (required for `stream_event` display) — no conditional paths
- Renderer doesn't suppress content under normal conditions

**Proposed fix:** Capture stderr instead of null'ing it. When the process exits, if there's stderr content, display it as a warning. Something like:
```
[warn] Claude stderr: <error message>
Claude process exited
```

**Questions:**
- Does this match what you're observing? (sessions that seem to start but show nothing before exiting?)
- Or is it more like sessions that run for a while and produce output that disappears?

---

## Done
- P1: Split main into main + review agents
- P1: First typed character after entering interactive with Ctrl+O seems to be swallowed
- P1: Thinking messages: only indent the "Thinking...", not the [N] before it
- P1: Add wait-for-user to worker and ralph system prompts
- P1: wait-for-user re-proposal
- P1: Simplify status line after exiting embedded interactive session
- P1: wait-for-user prompt final revision
