---
priority: P2
state: new
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
