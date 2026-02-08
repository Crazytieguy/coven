Issue: The steering VCR test doesn't demonstrate steering working — it uses a task too short for the steering message to take effect. Re-record with a longer multi-step task so the snapshot shows Claude responding to the steering input.
Status: draft

## Approach

The current steering test uses a short task ("Summarize this file") that completes before the steering message can influence Claude's behavior. The fix is to re-record the VCR with a longer, multi-step task where the steering message arrives mid-stream and visibly alters Claude's output.

### Steps

1. Update `tests/cases/steering.toml` to use a longer, multi-step prompt that gives Claude enough work to be interruptible — e.g., "Read each file in this directory, summarize each one, then write a combined report" or similar multi-tool task.
2. Update the steering message to redirect to something clearly different and verifiable in the output — e.g., "Actually, just count the lines in each file instead."
3. Set an appropriate `steering_delay_ms` to send the steering message after the first tool call but before the task completes.
4. Re-record with `cargo run --bin record-vcr steering`.
5. Run `cargo test` and review the snapshot — verify that Claude's output after the steering message reflects the redirected task (line counts instead of summaries, or whatever the steered task is).
6. Accept with `cargo insta accept` once the snapshot shows steering working.

### What to look for in the snapshot

The snapshot should show:
- Initial tool calls for the original task
- The steering message appearing in the stream
- Subsequent output from Claude that follows the steered instruction, not the original one

This validates both the display rendering AND that steering actually works.

## Questions

### What prompt and steering message should we use?

The prompt needs to be long enough that Claude is mid-stream when the steering message arrives, and the steering redirect needs to produce visibly different output. Suggestions:

- **Option A**: Prompt: "Read and summarize each file in this directory." Steer: "Actually, just count the lines in each file." (Clear difference: summaries vs line counts)
- **Option B**: Prompt: "Write a detailed analysis of this codebase." Steer: "Stop, just list the files instead." (Clear difference: analysis vs file listing)

The delay timing may need experimentation to hit the right window.

Answer:

## Review

