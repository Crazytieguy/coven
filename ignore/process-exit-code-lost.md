---
priority: P2
state: review
---

# ProcessExit event always carries None exit code

The session reader task in `src/session/runner.rs:221` always sends `AppEvent::ProcessExit(None)` when stdout closes — it has no access to the child process exit code. The actual exit code is only available via `runner.wait().await`, but every call site discards the result:

- `src/commands/run.rs:94` — `let _ = runner.wait().await;`
- `src/commands/run.rs:123` — `let _ = runner.wait().await;`
- `src/commands/ralph.rs:102` — `let _ = runner.wait().await;`
- `src/commands/session_loop.rs:984-985` — `let _ = runner.wait().await;`
- `src/fork.rs:139` — `let _ = runner.wait().await;`

This means `renderer.render_exit(code)` at `src/commands/session_loop.rs:321` always receives `None`, so the user sees "Claude process exited" without any exit code when the claude process crashes or exits abnormally.

## Impact

Non-zero exit codes from the claude CLI (OOM, crash, API errors) are silently swallowed, making it harder to diagnose production issues.

## Possible fix

After calling `runner.wait().await` and getting a non-zero exit code, render a line like "Claude process exited with code N". Alternatively, restructure so the reader task can report the exit code (e.g., by having the reader task also await the child status after stdout EOF).

## Plan

Move `Child` ownership into the reader task so it can await the real exit code after stdout closes. Use a oneshot channel for kill coordination.

### 1. Restructure `SessionRunner` fields (`src/session/runner.rs`)

Replace the `child: Option<Child>` field with:
- `kill_tx: Option<oneshot::Sender<()>>` — signals the reader task to kill the child
- `reader_handle: Option<JoinHandle<()>>` — handle to await reader task completion

```rust
pub struct SessionRunner {
    kill_tx: Option<oneshot::Sender<()>>,
    reader_handle: Option<JoinHandle<()>>,
    stdin: Option<ChildStdin>,
}
```

Update `stub()` to set all three fields to `None`.

### 2. Modify `spawn_reader` to own the child process (`src/session/runner.rs`)

Change signature to accept `Child` and `oneshot::Receiver<()>`, return `JoinHandle<()>`:

```rust
fn spawn_reader(
    stdout: ChildStdout,
    mut child: Child,
    mut kill_rx: oneshot::Receiver<()>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
) -> JoinHandle<()>
```

Inside the spawned task, use `tokio::select!` in the read loop to handle kill signals:

```rust
loop {
    tokio::select! {
        line = lines.next_line() => {
            match line {
                Ok(Some(line)) => { /* parse and send, same as today */ }
                _ => break, // EOF or error
            }
        }
        _ = &mut kill_rx => {
            child.kill().await.ok();
            let code = child.wait().await.ok().and_then(|s| s.code());
            let _ = event_tx.send(AppEvent::ProcessExit(code));
            return;
        }
    }
}
// After stdout EOF, await the real exit code
let code = child.wait().await.ok().and_then(|s| s.code());
let _ = event_tx.send(AppEvent::ProcessExit(code));
```

### 3. Update `spawn()` to wire up the new fields (`src/session/runner.rs`)

```rust
let (kill_tx, kill_rx) = tokio::sync::oneshot::channel();
let reader_handle = Self::spawn_reader(stdout, child, kill_rx, event_tx);

Ok(Self {
    kill_tx: Some(kill_tx),
    reader_handle: Some(reader_handle),
    stdin: Some(stdin),
})
```

### 4. Update `kill()` and `wait()` (`src/session/runner.rs`)

**`kill()`**: Send the kill signal, then await the reader handle for cleanup:
```rust
pub async fn kill(&mut self) -> Result<()> {
    if let Some(tx) = self.kill_tx.take() {
        tx.send(()).ok();
    }
    if let Some(handle) = self.reader_handle.take() {
        handle.await.ok();
    }
    Ok(())
}
```

**`wait()`**: Just await the reader handle. The real exit code has already been sent via the event channel, so this method is only for ensuring the process has fully exited:
```rust
pub async fn wait(&mut self) -> Result<Option<i32>> {
    if let Some(handle) = self.reader_handle.take() {
        handle.await.ok();
    }
    Ok(None)
}
```

All callers already discard the return value (`let _ = runner.wait().await;`), so returning `Ok(None)` is fine.

### 5. Suppress code 0 in `render_exit` (`src/display/renderer.rs`)

Currently `render_exit(Some(0))` would display "Claude process exited with code 0", which is noise for the common case. Update to only show the code when non-zero:

```rust
pub fn render_exit(&mut self, code: Option<i32>) {
    let msg = match code {
        Some(c) if c != 0 => format!("Claude process exited with code {c}"),
        _ => "Claude process exited".to_string(),
    };
    // ... rest unchanged
}
```

### 6. Re-record VCR fixtures and accept snapshots

Re-recording will change `ProcessExit(None)` to `ProcessExit(Some(0))` in VCR files for normal exits. With step 5, the rendered output stays the same ("Claude process exited" without the code) so snapshot diffs should be minimal (just the serialized event field).

Run `cargo run --bin record-vcr`, then `cargo test`, review diffs, and `cargo insta accept`.

### Files changed

- `src/session/runner.rs` — main restructuring (steps 1-4)
- `src/display/renderer.rs` — suppress code 0 display (step 5)
- VCR fixture files — re-recorded (step 6)
