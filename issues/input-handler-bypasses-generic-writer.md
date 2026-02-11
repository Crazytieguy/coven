---
priority: P1
state: new
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
