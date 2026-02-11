---
priority: P1
state: review
---

# Fork uses empty string when session_id is None

`src/commands/session_loop.rs:292` and `:384` — when a fork is triggered, the session ID is obtained via:

```rust
let session_id = state.session_id.clone().unwrap_or_default();
```

If `session_id` is `None` (e.g., before the first `Result` event sets it), this passes an empty string to `fork::run_fork()`, which builds a `SessionConfig` with `resume: Some("".to_string())`. This translates to `claude --resume ""` on the command line, which will likely error or produce undefined behavior.

**Impact:** If Claude emits a `<fork>` tag before the first `Result` event (unlikely but possible in edge cases), the fork children will fail to spawn or attach to a wrong session.

**Fix:** Return an error instead of silently using an empty string:

```rust
let session_id = state.session_id.clone()
    .context("cannot fork: no session ID yet")?;
```

## Plan

Two-line fix in `src/commands/session_loop.rs`:

1. Add `Context` to the existing anyhow import on line 4: `use anyhow::{Context, Result};`
2. Replace `state.session_id.clone().unwrap_or_default()` with `state.session_id.clone().context("cannot fork: no session ID yet")?` at both call sites (line 292 and line 384)
