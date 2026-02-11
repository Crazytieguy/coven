---
priority: P2
state: approved
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

## Plan

In `Renderer::set_title` (`src/display/renderer.rs:760`), sanitize the `title` parameter before embedding it in the OSC sequence. Strip all C0 control characters (`\x00`–`\x1f`) and DEL (`\x7f`) from the string using `retain` or `replace`:

```rust
pub fn set_title(&mut self, title: &str) {
    let sanitized: String = title.chars().filter(|c| !c.is_ascii_control()).collect();
    queue!(self.out, Print(format!("\x1b]2;{sanitized}\x07"))).ok();
    self.out.flush().ok();
}
```

This is a single-line addition. No new dependencies, no new modules, no tests needed (the risk is terminal display corruption from LLM-generated content, which is not unit-testable in a meaningful way).
