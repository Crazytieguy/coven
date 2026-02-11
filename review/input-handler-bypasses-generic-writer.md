---
priority: P1
state: review
---

# InputHandler writes directly to stdout, bypassing generic writer

`src/display/input.rs:114-162` (`redraw()`) and lines 166-186 (`clear_input_lines()`) write directly to `io::stdout()`:

```rust
pub fn redraw(&mut self) {
    let mut out = io::stdout();
    // ... all writes go to stdout
}
```

The `Renderer<W>` is generic over `W: Write` — tests use `Vec<u8>` to capture output. But `InputHandler` is a separate struct with no writer parameter, so its output always goes to the real stdout. This creates an architectural inconsistency:

1. **Input rendering can't be captured in tests** — any future unit test for input editing behavior would have side effects on stdout rather than writing to a test buffer.
2. **Coupling to stdout** — the renderer's generic writer abstraction is undermined when half the display pipeline bypasses it.

This doesn't cause test failures today because VCR replay doesn't exercise the input handler (no terminal events during replay). But it prevents writing proper unit tests for the input editing experience (cursor movement, line wrapping, word deletion rendering, etc.).

## Fix

Add a writer parameter to `InputHandler` (or share the renderer's writer via a reference/`Rc<RefCell<W>>`). The simplest approach: make `InputHandler` generic over `W: Write`, pass the writer at construction time, and use it in `redraw()` and `clear_input_lines()` instead of `io::stdout()`.

## Plan

Make `InputHandler` generic over `W: Write`, mirroring the `Renderer<W>` pattern. The struct stores its own writer and uses it everywhere instead of `io::stdout()`.

### 1. Make `InputHandler<W: Write>` in `src/display/input.rs`

- Add `out: W` field to the struct:
  ```rust
  pub struct InputHandler<W: Write = io::Stdout> {
      // ... existing fields ...
      out: W,
  }
  ```
- Change `impl InputHandler` → `impl<W: Write> InputHandler<W>`
- Update `new()` to accept a writer:
  ```rust
  pub fn new(prefix_width: usize, writer: W) -> Self {
      Self { ..., out: writer }
  }
  ```
- In `redraw()` (line 114): replace `let mut out = io::stdout();` with `let out = &mut self.out;` — all the `queue!(out, ...)` and `out.flush()` calls stay the same
- In `clear_input_lines()` (line 166): same replacement — `let out = &mut self.out;` instead of `let mut out = io::stdout();`
- Remove the `use std::io::{self, Write}` → keep `use std::io::{self, Write}` (still needed for the default type parameter and `Write` trait)

### 2. Update construction sites (3 files)

Each command creates `InputHandler::new(2)` — add `io::stdout()`:

- **`src/commands/run.rs:33`**: `InputHandler::new(2)` → `InputHandler::new(2, io::stdout())`
- **`src/commands/ralph.rs:56`**: same
- **`src/commands/worker.rs:109`**: same

Add `use std::io;` to each file if not already imported.

### 3. Update function signatures that take `&mut InputHandler`

Every function that takes `input: &mut InputHandler` already has `W: Write` in scope (they're generic over `W` for the `Renderer<W>` parameter). Change `&mut InputHandler` → `&mut InputHandler<W>`:

- **`src/commands/session_loop.rs`** — 5 functions:
  - `run_session` (line 59)
  - `handle_session_key_event` (line 188)
  - `wait_for_followup` (line 409)
  - `wait_for_user_input` (line 432)
  - `wait_for_text_input` (line 445)
- **`src/commands/worker.rs`** — 2 sites:
  - `PhaseContext` struct (line 28): `input: &'a mut InputHandler` → `input: &'a mut InputHandler<W>`
  - `pick_issue_interactive` (line 1100)
- **`src/commands/run.rs`** (line 126): function taking `&mut InputHandler`

### 4. Verify

- `cargo fmt`
- `cargo clippy`
- `cargo test`
