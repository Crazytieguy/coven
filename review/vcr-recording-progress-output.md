---
priority: P1
state: review
---

# Add progress output to VCR recording

`cargo run --bin record-vcr` runs silently — there's no way to tell what's happening during recording. This makes it painful to use from automated workers or when recording slow multi-step orchestration tests.

The recorder should stream some lightweight progress output so you can tell:
- Which test case is currently being recorded
- Which step (for multi-step tests) is active
- Key events as they happen (dispatch started, agent session started, landing, etc.)

This could use the coven display format or a simpler structured log. The key requirement is that a human or agent watching the output can tell whether recording is progressing or stuck.

## Plan

Add a progress callback to `VcrContext` that fires on every `push_entry` during recording, and use it in `record-vcr` to print structured progress lines to stderr.

### 1. Add progress callback to `VcrContext`

In `src/vcr.rs`:

- Add a new field to `VcrContext`: `progress_callback: Option<Box<dyn Fn(&str, &Value)>>` — receives `(label, result)` on each recorded entry.
- In `push_entry` (line ~305), after pushing to the entries vec and notifying the trigger controller, call the progress callback if set.
- Add a builder method `with_progress(cb: impl Fn(&str, &Value) + 'static) -> Self` that sets the callback. This composes with the existing `record_with_triggers` constructor — apply it after construction.

### 2. Define a progress printer in `record_vcr.rs`

Add a `ProgressPrinter` struct that:
- Stores the test case name (and optionally step name for multi-step).
- Implements a method `fn callback(&self) -> impl Fn(&str, &Value)` that maps VCR labels to human-readable progress lines on stderr.
- Uses `eprintln!` with a prefix like `[{name}]` or `[{name}/{step}]` for disambiguation when cases run concurrently.

**Label-to-message mapping** (only emit for "milestone" labels, skip noisy ones like `next_event`):

| VCR label | Progress message |
|-----------|-----------------|
| `worker_paths` | `Resolving worker paths...` |
| `worktree::spawn` | `Spawning worktree...` |
| `worker_state::register` | `Worker registered` |
| `worker_state::read_all` | `Reading issue queue...` |
| `agents::load_agents` | `Loading agents...` |
| `spawn` | `Starting session...` |
| `send_message` | `Sending message...` |
| `worker_state::deregister` | `Worker deregistered` |
| `worktree::remove` | `Removing worktree...` |
| `current_dir` | (skip) |
| `next_event` | (skip — too noisy) |
| anything else | `{label}` (pass through for new labels) |

### 3. Wire up the progress callback in recording functions

In `record_vcr.rs`, after creating each `VcrContext::record_with_triggers(...)` or `VcrContext::record()`:

- **`record_case`** (~line 228): Create `ProgressPrinter::new(name)`, call `vcr.set_progress(printer.callback())`.
- **`record_multi_step`** (~line 383/398): Create `ProgressPrinter::new_step(test_name, step.name)`, call `vcr.set_progress(printer.callback())`.

### 4. Add top-level case start/end messages

In `main()`, before spawning each task (~line 103), print `Recording {name}...` to stderr. The existing `Done: {name}.vcr` / `FAILED: {name}` messages at lines 113-121 already handle completion — keep those.

### 5. Summary of files changed

- `src/vcr.rs` — Add optional progress callback field, `set_progress` method, call from `push_entry`.
- `src/bin/record_vcr.rs` — Add `ProgressPrinter`, wire callbacks into recording functions, add start messages.

### Design notes

- **Callback on `VcrContext` vs. separate channel**: A callback is simpler — no new async machinery, no new types, and `push_entry` already exists as the single chokepoint. The callback runs synchronously inside `push_entry`, which is fine since it just does `eprintln!`.
- **Why stderr**: Recording output (VCR files) goes to the filesystem, so stderr is the natural place for progress. It also interleaves correctly when multiple cases run concurrently.
- **No test changes needed**: The progress callback is `Option`-al and only set during recording. Replay and live modes are unaffected. No VCR re-recording needed.
