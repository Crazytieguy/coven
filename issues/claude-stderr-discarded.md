---
priority: P2
state: new
---

# Claude process stderr silently discarded

## Problem

In `src/session/runner.rs:71`, the claude subprocess stderr is piped to null:

```rust
cmd.stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
```

If the claude CLI writes warnings or error messages to stderr (e.g., startup failures, deprecation notices, permission errors), they are silently lost. This makes it difficult to diagnose issues when a session fails to start or behaves unexpectedly — the only signal is a `ProcessExit` event with no context.

## Possible approaches

1. **Capture and display**: Pipe stderr and forward non-empty lines to the renderer as warnings after the process exits
2. **Capture and log on failure**: Only display stderr content when the process exits with a non-zero code
3. **Pipe to parent stderr**: Use `Stdio::inherit()` for stderr, letting it appear in the terminal. May interfere with raw mode terminal display

Option 2 seems safest — avoids noise during normal operation while preserving diagnostics on failure.
