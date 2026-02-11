---
priority: P2
state: review
---

# InputHandler writes directly to stdout, bypassing Renderer's writer

`src/display/input.rs` has two methods that write directly to `io::stdout()` instead of using the `Renderer<W>`'s configurable writer:

- `redraw()` (line 117): `let mut out = io::stdout();`
- `clear_input_lines()` (line 169): `let mut out = io::stdout();`

The `Renderer<W>` is generic over `W: Write`, allowing tests to capture output to `Vec<u8>`. But `InputHandler` bypasses this, writing cursor movement and input line content directly to the real stdout.

## Impact

- During VCR test replay (where the renderer writes to `Vec<u8>`), input display goes to stdout instead of the captured output buffer, so input-related rendering is never snapshot-tested.
- The `InputHandler` can't be given a `&mut W` because it doesn't have access to the renderer's writer.

## Possible fix

Thread a writer reference through `InputHandler`, or give `InputHandler` its own `W: Write` generic parameter, or extract the redraw logic into the renderer where the writer is available.

## Plan

**Approach:** Thread `&mut impl Write` as a parameter through `InputHandler` methods that produce terminal output, rather than making the struct generic (which would add lifetime/type complexity everywhere) or moving logic into the renderer (which would break InputHandler's self-contained editing encapsulation).

### 1. Add a `writer()` accessor to `Renderer<W>`

In `src/display/renderer.rs`, add a public method:

```rust
pub fn writer(&mut self) -> &mut W {
    &mut self.out
}
```

This lets callers in `session_loop.rs` borrow the renderer's writer to pass into `InputHandler` methods.

### 2. Add `out: &mut impl Write` parameter to `InputHandler` methods

In `src/display/input.rs`, change the signatures of every method that writes to the terminal or calls a method that does. The dependency chain is:

- `redraw(&mut self)` → `redraw(&mut self, out: &mut impl Write)` — remove `let mut out = io::stdout();`
- `clear_input_lines(&self)` → `clear_input_lines(&self, out: &mut impl Write)` — remove `let mut out = io::stdout();`
- `insert_char(&mut self, c: char)` → `insert_char(&mut self, c: char, out: &mut impl Write)` — pass `out` to `redraw()`
- `delete_range(&mut self, ...)` → `delete_range(&mut self, ..., out: &mut impl Write)` — pass `out` to `redraw()`
- `move_cursor(&mut self, pos: usize)` → `move_cursor(&mut self, pos: usize, out: &mut impl Write)` — pass `out` to `redraw()`
- `handle_key(&mut self, event: &KeyEvent)` → `handle_key(&mut self, event: &KeyEvent, out: &mut impl Write)` — pass `out` to all internal editing methods
- `handle_inactive_key(&mut self, event: &KeyEvent)` → `handle_inactive_key(&mut self, event: &KeyEvent, out: &mut impl Write)` — pass `out` to `insert_char()`
- `handle_enter(&mut self, event: &KeyEvent)` → `handle_enter(&mut self, event: &KeyEvent, out: &mut impl Write)` — pass `out` to `clear_input_lines()`

Remove `use std::io::{self, Write};` and replace with `use std::io::Write;` (no more `io::stdout()` references).

### 3. Update call sites in `session_loop.rs`

There are 4 external call sites, all in `src/commands/session_loop.rs`:

1. **`handle_session_key_event` (line 196):**
   ```rust
   let action = input.handle_key(key_event, renderer.writer());
   ```

2. **`handle_session_key_event` (line 200, Activated branch):**
   ```rust
   input.redraw(renderer.writer());
   ```

3. **`wait_for_text_input` (line 508):**
   ```rust
   let action = input.handle_key(&key_event, renderer.writer());
   ```

4. **`wait_for_text_input` (line 529, Activated branch):**
   ```rust
   input.redraw(renderer.writer());
   ```

### 4. Verify

- `cargo fmt && cargo clippy && cargo test` — ensure no regressions.
- Confirm `io::stdout` no longer appears in `src/display/input.rs`.
