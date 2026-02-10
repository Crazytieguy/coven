Issue: [P1] Consider removing the explicit "mode" system for test case toml files. Instead, just support key combinations generically, ideally using a library. Plan file should either include an implementation plan or a convincing reason not to do this.
Status: draft

## Approach

**Recommendation: keep the mode system**, with a minor rename for clarity.

### Why a generic key sequence system isn't worth it

The current mode enum maps exactly to coven's four interaction patterns:

| Mode        | Keys injected                          | Semantic meaning       |
|-------------|----------------------------------------|------------------------|
| `followup`  | chars + Alt+Enter                      | Queue message for next turn |
| `steering`  | chars + Enter                          | Redirect mid-stream    |
| `interrupt`  | Ctrl+C, then chars + Enter            | Kill process, resume   |
| `exit`      | Ctrl+D                                 | End session            |

A generic key sequence system would need to solve: "where does the message text go in the sequence?" Followup/steering inject text *before* the submit key. Interrupt injects a control key *before* the text. Exit has no text at all. This means either:

1. **A placeholder token** like `keys = "Ctrl+C {text} Enter"` — works but is a custom mini-language that still needs a parser, just with worse ergonomics than an enum.
2. **Separate `pre_keys` / `post_keys` fields** — more flexible but more verbose and error-prone.
3. **Raw crossterm event arrays** — maximally generic but terrible DX for test authors.

None of these are an improvement over `mode = "interrupt"`.

**No existing library** cleanly solves "parse human-readable key combo strings into crossterm KeyEvents." We'd end up writing a small parser regardless, and it would only need to support the same 4 patterns the enum already handles.

The modes also provide **validation**: you can't accidentally write a meaningless key sequence. The enum constrains the input to exactly the patterns the application supports.

### What we should do instead

Rename `mode` to `input` across test TOML files and code. The word "mode" is confusing — it reads like it's the test case's mode rather than the type of input event. `input = "steering"` is clearer.

Changes:
1. `TriggerInputMode` → `TriggerInput` (in `src/vcr.rs`)
2. `mode` field → `input` field (in `TestMessage` struct and all TOML files)
3. Update documentation/comments

This is a small rename, no behavior change.

## Questions

### Is the rename worthwhile or too trivial?

The rename from `mode` to `input` is a low-stakes clarity improvement. But it touches all test TOML files. If you'd rather leave it as-is entirely (accepting the mode system is fine as-is), that works too.

Answer:
