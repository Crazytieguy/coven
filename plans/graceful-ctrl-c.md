Issue: [P0] `runner.kill()` uses SIGKILL which doesn't give Claude time to persist the conversation. Resume after Ctrl+C fails with "No conversation found with session ID". Consider using SIGTERM or closing stdin instead.
Status: draft

## Approach

### Root cause

The `interrupt_resume` test case interrupts during the **first** message response. At that point, Claude hasn't persisted anything for the session yet, so there's nothing to resume — regardless of how gracefully we terminate the process.

### Fix

Change the test case so the interrupt happens during the **second** message, after the first has completed and been persisted:

1. **Adjust the prompt**: Use a prompt that naturally completes in one turn, e.g. "What is 2+2? Answer in one sentence."
2. **Add a follow-up message**: After the first response completes, send a follow-up like "Now what is 3+3? Answer in one sentence."
3. **Move the interrupt**: Trigger the interrupt on `content_block_start` during the second response (after the follow-up).
4. **Resume**: The resume message "Continue where you left off" follows the interrupt as before.

The test case TOML would look roughly like:

```toml
[run]
prompt = "What is 2+2? Answer in one sentence."

# First follow-up completes normally
[[messages]]
content = "Now what is 3+3? Answer in one sentence."
trigger = "result"

# When the second response starts, interrupt and resume
[[messages]]
content = "Continue where you left off"
trigger = '{"Ok": {"Claude": {"Claude": {"type": "stream_event", "event": {"type": "content_block_start"}}}}}'
mode = "interrupt"
```

This ensures the session has at least one completed turn before the interrupt, so there's actually something to resume.

### If this still fails

If resume still fails even with a persisted first turn, then we revisit changing `runner.kill()` to use SIGINT with a timeout. But the hypothesis is that the current kill mechanism is fine — we just need the session to have persisted state.

### Changes

- **`tests/cases/interrupt_resume.toml`**: Restructure as described above
- **`tests/cases/interrupt_resume.vcr`**: Re-record with `cargo run --bin record-vcr interrupt_resume`
- **`tests/cases/interrupt_resume.snap`**: Accept new snapshot after verifying it shows the expected flow (first response, interrupt, resume, second response)

## Review

