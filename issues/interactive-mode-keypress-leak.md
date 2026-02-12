---
priority: P1
state: approved
---

# Interactive mode (Ctrl+o) leaks keypresses to coven

When entering interactive mode via Ctrl+o, roughly half of keypresses are being sent to coven instead of the child Claude Code instance. Claude Code should fully take over the terminal until it exits — all keypresses should make it to Claude Code with none intercepted by coven.

## Plan

### Root Cause

`create_live_io()` in `main.rs:130` spawns a background tokio task that continuously reads from crossterm's `EventStream` and forwards events into `term_rx`. When `open_interactive_session()` spawns a child `claude --resume` process, this background reader **keeps running** and competes with the child for stdin reads at the kernel level. Roughly half the keypresses go to each process.

The existing `tcflush(STDIN_FILENO, TCIFLUSH)` after the child exits only clears the kernel's terminal input buffer — events already consumed by the background reader and queued in the mpsc channel are unaffected.

### Fix

Add a pause/resume mechanism to the background terminal reader so it can release stdin entirely while the child process runs.

#### 1. Add a pause gate to the background reader (`main.rs`)

- Add a `tokio::sync::watch` channel (`term_gate_tx`, `term_gate_rx`) with initial value `true` (running).
- In the background reader task, before each `stream.next().await`, check the gate. If paused (`false`), **drop the `EventStream`** (which releases the underlying stdin reader) and wait for the gate to become `true` again, then recreate the `EventStream`.
- Pass `term_gate_tx` into `Io` (new field) so callers can pause/resume.

#### 2. Add pause/resume methods to `Io` (`vcr.rs`)

- Add a `term_gate: Option<tokio::sync::watch::Sender<bool>>` field to `Io` (set to `Some` for live, `None` for dummy/replay).
- `pub fn pause_term_reader(&self)` — sends `false` to the watch channel.
- `pub fn resume_term_reader(&self)` — sends `true` to the watch channel.
- `pub fn drain_term_events(&mut self)` — drains `term_rx` of any residual events that were queued before the pause took effect.

#### 3. Gate the reader around interactive sessions (`session_loop.rs`)

In `open_interactive_session()`, before disabling raw mode:
1. Call `io.pause_term_reader()` to signal the background task to drop its `EventStream`.
2. Yield briefly (`tokio::task::yield_now()`) to give the background task time to drop the stream.

After the child exits and before re-enabling raw mode:
1. Call `io.drain_term_events()` to clear any residual events.
2. Call `io.resume_term_reader()` to restart the `EventStream`.

#### 4. Thread `io` into `open_interactive_session`

`open_interactive_session()` currently doesn't take `&mut Io`. Add it as a parameter and update the call site in `wait_for_interrupt_input()` (line 481).

### Files Changed

- `src/main.rs` — watch channel creation, background reader pause gate logic
- `src/vcr.rs` — new `term_gate` field on `Io`, `pause_term_reader`/`resume_term_reader`/`drain_term_events` methods
- `src/commands/session_loop.rs` — `open_interactive_session` gains `&mut Io`, pause/resume calls
