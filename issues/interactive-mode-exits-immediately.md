---
priority: P1
state: approved
---

# Interactive mode exits immediately instead of waiting for user prompt

Running `coven` without arguments should start an interactive session where it waits for the user to type a prompt at stdin. Instead, it exits immediately.

Expected behavior: `coven` with no arguments waits for user input, then streams the response.
Actual behavior: `coven` with no arguments exits right away.

## Plan

### Root Cause

In `create_live_io()` (`src/main.rs:127`), the Claude event sender `_event_tx` is created and immediately dropped:

```rust
let (_event_tx, event_rx) = mpsc::unbounded_channel();
```

When `wait_for_text_input()` calls `io.next_event()`, `tokio::select!` races `event_rx.recv()` against `term_rx.recv()`. Since the sender was dropped, `event_rx.recv()` returns `None` immediately. In `next_event()` (`src/vcr.rs:398`), this `None` is converted to `ProcessExit(None)`, which `wait_for_text_input()` treats as a signal to exit (`src/commands/session_loop.rs:534`).

### Fix

Call `io.clear_event_channel()` in `create_live_io()` after constructing the `Io`. This method (already exists at `src/vcr.rs:426-430`) replaces the event channel with a fresh one whose sender is kept alive via `idle_tx`, so `recv()` blocks instead of returning `None`.

**`src/main.rs` — `create_live_io()`:**

```rust
fn create_live_io() -> (Io, VcrContext) {
    // ... existing channel setup and spawn ...

    let mut io = Io::new(event_rx, term_rx);
    // Keep the event channel alive so recv() blocks instead of
    // returning ProcessExit immediately (the sender was dropped above).
    io.clear_event_channel();
    let vcr = VcrContext::live();
    (io, vcr)
}
```

This is a one-line change. The unused initial `_event_tx`/`event_rx` pair could also be cleaned up (replaced with a single `clear_event_channel()` call), but that's optional cosmetic cleanup.

### Verification

Run `cargo build && cargo test` to confirm no regressions. Manual test: run `coven` with no arguments and confirm it waits for input.
