Issue: [P0] `runner.kill()` uses SIGKILL which doesn't give Claude time to persist the conversation. Resume after Ctrl+C fails with "No conversation found with session ID". Consider using SIGTERM or closing stdin instead.
Status: draft

## Approach

### Root cause

The `interrupt_resume` test case interrupts during the **first** message response. At that point, Claude hasn't persisted anything for the session yet, so there's nothing to resume — regardless of how gracefully we terminate the process.

### Fix

Use a prompt that triggers tool calls within a single turn. Claude persists the session file after every message, so after the first tool call round-trip completes, the session is resumable. No need for multiple user turns.

1. **Prompt**: Something like "List the files in the current directory, then read README.md" — this naturally causes a chain of tool calls within one turn.
2. **Interrupt trigger**: Fire on a stream event that occurs *after* at least one tool call has completed (e.g., the second `content_block_start`, or a `tool_use` event after the first tool result). The exact trigger will depend on what events flow through after the first tool round-trip.
3. **Resume**: "Continue where you left off" as before.

The test case TOML would look roughly like:

```toml
[run]
prompt = "List the files in the current directory, then read README.md"

# Interrupt after the first tool call completes and Claude starts its next action
[[messages]]
content = "Continue where you left off"
trigger = '<event indicating second tool call or post-tool-result response>'
mode = "interrupt"
```

The exact trigger event needs to be determined during implementation by inspecting the VCR recording to find an event that reliably fires after the first tool call round-trip.

### If this still fails

If resume still fails even with a persisted tool call turn, then we revisit changing `runner.kill()` to use SIGINT with a timeout.

### Changes

- **`tests/cases/interrupt_resume.toml`**: New prompt and trigger as described above
- **`tests/cases/interrupt_resume.vcr`**: Re-record with `cargo run --bin record-vcr interrupt_resume`
- **`tests/cases/interrupt_resume.snap`**: Accept new snapshot after verifying it shows interrupt + successful resume

## Review

