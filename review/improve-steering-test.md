---
priority: P2
state: review
---

# Improve steering test to verify model follows redirection

The current steering test (`steering.vcr`) sends a mid-stream steering instruction ("Actually, just count the lines in each file instead") but the model ignores it and continues with the original task (file summaries).

The steering arrives during a `content_block_start` (tool use) trigger, which means it likely reaches the model after the most recent batch of tool calls has already been submitted. By the time the model sees the steering, it's committed to its current plan.

## Desired Outcome

The steering test should demonstrate that mid-stream steering actually redirects the model's behavior. This requires:

1. A task that takes multiple tool call batches (not just one batch of reads)
2. Steering timed to arrive between batches, so the model can process it before committing to the next batch

## Context

- Snapshot: `tests/cases/session/steering/steering.snap`
- VCR fixture: `tests/cases/session/steering/steering.vcr`
- Found during snapshot audit (issue `audit-snapshot-tests.md`, finding M1)

## Plan

### Root cause

Two factors cause the steering to arrive too late:

1. **Trigger fires too early.** The trigger matches the first `content_block_start` with `type: tool_use`, which is the Glob tool call at the very start of Turn 1 (VCR line 40). This is the earliest tool call in the sequence — the model is still deciding what to do.

2. **Steering text is long.** "Actually, just count the lines in each file instead" is 51 characters. Each character is injected as a separate terminal key event, interleaved one-per-`next_event` call with Claude streaming events. This adds ~100 event loop iterations of latency between trigger fire and the actual `send_message`.

Combined: the trigger fires at VCR line 40 (Glob tool_use), but `send_message` doesn't happen until line 144 — after Turn 3 has already started (message_start at line 141, thinking at line 142). The model has committed to its summary plan by the time it sees the steering.

### Fix: change trigger + shorten text

**1. Change the trigger to fire on the first `user` event (tool result delivery).**

Current trigger (fires on first tool_use block = Glob, very early in Turn 1):
```
trigger = '{"Ok": {"Claude": {"Claude": {"type": "stream_event", "event": {"type": "content_block_start", "content_block": {"type": "tool_use"}}}}}}'
```

New trigger (fires when the first tool result comes back = Glob result, at the Turn 1→2 boundary):
```
trigger = '{"Ok": {"Claude": {"Claude": {"type": "user"}}}}'
```

This is the same trigger pattern used by the `interrupt_resume` test, whose TOML even documents it: *"The `type: user` event in the stream represents a tool result, not the initial user message."*

Firing at the turn boundary gives the injected characters the entire Turn 2 streaming window (thinking + text + Read tool call — roughly 80 events) to complete before the next turn boundary.

**2. Shorten the steering text.**

Change from: `"Actually, just count the lines in each file instead"` (51 chars)
To: `"count lines instead"` (19 chars)

Fewer characters means fewer event loop iterations between trigger fire and `send_message`. With the later trigger plus shorter text, the `send_message` should arrive well within Turn 2's streaming window — before the next API request is sent.

**3. Keep the task and files unchanged.** The current prompt already produces multiple sequential tool call batches (Glob → Read file 1 → Read file 2 → Read file 3). No changes needed.

### Steps

1. Edit `tests/cases/session/steering/steering.toml`:
   - Change `trigger` to `'{"Ok": {"Claude": {"Claude": {"type": "user"}}}}'`
   - Change `content` to `"count lines instead"`
2. Re-record: `cargo run --bin record-vcr steering` (1-minute timeout)
3. Run `cargo test` — check snapshot diff
4. Verify: the snapshot should show the model switching from summaries to line counting after the steering message appears
5. If it works: `cargo insta accept`, then `cargo test` + `cargo clippy` to confirm clean
6. If the model still ignores steering: try the two-trigger approach (see fallback below)

### Fallback: two-trigger approach

If the single-trigger fix doesn't work (model still ignores steering), split into two triggers to decouple typing from submission:

```toml
[[messages]]
content = "count lines instead"
trigger = '{"Ok": {"Claude": {"Claude": {"type": "stream_event", "event": {"type": "content_block_start", "content_block": {"type": "tool_use"}}}}}}'
mode = "typing"

[[messages]]
content = ""
trigger = '{"Ok": {"Claude": {"Claude": {"type": "user"}}}}'
mode = "steering"
```

- First trigger (typing mode): fires early on the Glob tool_use, pre-types the text without Enter. Characters are injected over ~20 event iterations.
- Second trigger (steering mode, empty content): fires on the tool result, sends just the Enter key (1 event), submitting the pre-typed text immediately.

This ensures the text is fully typed before the critical moment, and Enter fires at the exact turn boundary.
