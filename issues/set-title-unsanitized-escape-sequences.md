---
priority: P2
state: new
---

# `set_title` doesn't sanitize input for terminal escape sequences

`Renderer::set_title` in `src/display/renderer.rs:761` writes an OSC escape sequence directly:

```rust
queue!(self.out, Print(format!("\x1b]2;{title}\x07"))).ok();
```

The `title` string is not sanitized. If it contains `\x07` (BEL) or `\x1b` (ESC), it could prematurely terminate the OSC sequence or inject additional escape sequences.

**How untrusted data reaches this function:**

In `src/commands/worker.rs:378-384`, the title is built from dispatch decision arguments:
```rust
let title_suffix = if args_display.is_empty() {
    agent
} else {
    format!("{agent} {args_display}")
};
ctx.renderer.set_title(&format!("cv {title_suffix} — {branch}"));
```

`args_display` is composed from key=value pairs where values come from the dispatch agent's LLM-generated YAML output (`src/dispatch.rs:50-61`). A model could emit values containing escape characters.

**Impact:** Low — the worst case is garbled terminal title or minor terminal display corruption. No command execution risk.

**Fix:** Strip or replace control characters (`\x00-\x1f`, `\x7f`) from the title string before embedding it in the OSC sequence.
