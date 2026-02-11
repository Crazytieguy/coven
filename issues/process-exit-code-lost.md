---
priority: P2
state: new
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
