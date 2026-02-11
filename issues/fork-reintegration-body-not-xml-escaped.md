---
priority: P1
state: new
---

# Fork reintegration message doesn't XML-escape body text

## Problem

In `src/fork.rs:188-209`, `compose_reintegration_message` escapes the `<task>` label attribute but does **not** escape the body text or error text:

```rust
let safe_label = label
    .replace('&', "&amp;")
    .replace('<', "&lt;")
    .replace('>', "&gt;")
    .replace('"', "&quot;");
match outcome {
    Ok(text) => {
        let _ = write!(xml, "<task label=\"{safe_label}\">\n{text}\n</task>\n");
    }
    Err(err) => {
        let _ = write!(xml, "<task label=\"{safe_label}\" error=\"true\">\n{err}\n</task>\n");
    }
}
```

If a fork child's result contains `</task>` or `</fork-results>`, it corrupts the XML structure of the reintegration message sent back to the parent Claude session. The parent model would see a malformed `<fork-results>` block where task boundaries are wrong, potentially attributing one child's output to another or losing results entirely.

The existing test at line 301-308 (`compose_reintegration_message_handles_angle_brackets`) demonstrates this — it asserts that `Vec<String>` appears unescaped in the body, which means `</task>` in child output would break parsing.

## Impact

Medium — fork child outputs frequently contain code with angle brackets (generic types, HTML, XML). While `</task>` specifically is unlikely in typical output, it's not impossible (e.g., a child discussing the fork protocol itself, or producing XML-like content). The inconsistency between escaped labels and unescaped bodies is a correctness gap.

## Fix

Escape `<`, `>`, and `&` in the body text, matching the label escaping. The body doesn't need `"` escaping since it's element content, not an attribute value.
