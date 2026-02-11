---
priority: P2
state: review
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

## Plan

**Approach:** Variant of option 1 — capture stderr and display when non-empty. In practice this is equivalent to option 2 since `claude` doesn't write to stderr on success, but it avoids depending on exit code detection (which is a separate issue — `ProcessExit` currently always carries `None` for the exit code).

### 1. Change `AppEvent::ProcessExit` to carry stderr content (`src/event.rs`)

Change from tuple variant to struct variant:

```rust
ProcessExit(Option<i32>),
```
→
```rust
ProcessExit {
    code: Option<i32>,
    stderr: String,
},
```

### 2. Pipe and read stderr (`src/session/runner.rs`)

- Line 71: change `Stdio::null()` to `Stdio::piped()`
- After taking stdout/stdin, also take `child.stderr`
- Spawn a small async task that reads all stderr lines into a `String` (using `BufReader::lines()`, joining with `\n`), returning via `JoinHandle<String>`
- Add `ChildStderr` to the imports from `tokio::process`
- Change `spawn_reader` signature to accept an additional `JoinHandle<String>` parameter for the stderr task
- In `spawn_reader`, after the stdout reading loop completes (stdout EOF), `.await` the stderr handle and include the result in the `ProcessExit` event:
  ```rust
  let stderr = stderr_handle.await.unwrap_or_default();
  let _ = event_tx.send(AppEvent::ProcessExit { code: None, stderr });
  ```

### 3. Update `render_exit` (`src/display/renderer.rs:744-751`)

- Change signature to `render_exit(&mut self, code: Option<i32>, stderr: &str)`
- Keep existing exit message logic
- After the exit message, if `stderr` is non-empty, display each line using `theme::error()` styling with a `[stderr]` prefix

### 4. Update all pattern-match sites

Each site currently destructures `ProcessExit(code)` or `ProcessExit(_)`. Update to `ProcessExit { code, stderr }` or `ProcessExit { .. }`:

- `src/commands/session_loop.rs:320` — pass both `code` and `&stderr` to `render_exit`
- `src/commands/session_loop.rs:358` — same
- `src/commands/session_loop.rs:530` — uses `ProcessExit(_)`, change to `ProcessExit { .. }`
- `src/fork.rs:125` — uses `ProcessExit(_)`, change to `ProcessExit { .. }`
- `src/vcr.rs:398, 404` — fallback constructions, change to `ProcessExit { code: None, stderr: String::new() }`

### 5. Re-record VCR fixtures and update snapshots

The serialized shape of `ProcessExit` changes from `{"ProcessExit": null}` to `{"ProcessExit": {"code": null, "stderr": ""}}`. All `.vcr` files referencing this event need re-recording:

```
cargo run --bin record-vcr
cargo test
cargo insta accept  # if snapshot diffs look correct
```

### Out of scope

The exit code is never captured by the reader task (`ProcessExit` always carries `code: None`). Fixing exit code capture is a separate concern — the struct variant introduced here gives it a natural home for a future change.
