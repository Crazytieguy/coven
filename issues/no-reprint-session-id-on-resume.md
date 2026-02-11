---
priority: P2
state: approved
---

# Don't re-print session ID when resuming a session

When resuming an existing session, the session ID is printed again redundantly. Since the user already saw it when the session started, there's no need to print it a second time on resume.

## Plan

### Root cause

The session header reprints because `state.session_id` is `None` when the resumed session's `Init` event arrives, so the `same_session` check in `src/lib.rs:31` evaluates to `false`.

The bug is in the resume handling pattern used in three places. The flow:

1. `state.session_id.take()` extracts the session ID (setting `state.session_id` to `None`)
2. The extracted ID is **moved** into `resume_with(text, session_id)`
3. `state.session_id.clone()` clones `None` (since `take()` already cleared it)
4. State is reset to default and `None` is assigned back

When the `Init` event arrives for the resumed session, `state.session_id` is `None`, so `same_session` is `false`, and `render_session_header` fires again.

### Fix

In all three resume sites, clone the session ID before moving it into `resume_with`, then restore it on the fresh state:

**`src/commands/run.rs` (lines 105-109):**
```rust
let session_cfg = base_session_cfg.resume_with(text, session_id.clone());
runner = session_loop::spawn_session(session_cfg, io, vcr).await?;
state = SessionState::default();
state.session_id = Some(session_id);
```

**`src/commands/ralph.rs` (lines 135-139):**
```rust
let resume_config = session_config.resume_with(text, session_id.clone());
runner = session_loop::spawn_session(resume_config, io, vcr).await?;
state = SessionState::default();
state.session_id = Some(session_id);
```

**`src/commands/worker.rs` (lines 1003-1008):**
```rust
let resume_config = session_config.resume_with(text, session_id.clone());
runner = session_loop::spawn_session(resume_config, ctx.io, ctx.vcr).await?;
state = SessionState::default();
state.session_id = Some(session_id);
```

Each change is two lines: add `.clone()` to the `resume_with` call, and replace the `prev_session_id` dance with `Some(session_id)`.
