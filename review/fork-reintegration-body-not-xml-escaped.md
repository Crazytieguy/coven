---
priority: P1
state: review
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

## Plan

In `src/fork.rs`, `compose_reintegration_message` (line 184):

1. **Extract an `xml_escape_content` helper** (local to the function or a private fn) that escapes `&` → `&amp;`, `<` → `&lt;`, `>` → `&gt;` for element body text. The existing label escaping also does `"` → `&quot;` which isn't needed for element content, so keep them separate. A simple approach: add a private `fn xml_escape_content(s: &str) -> String` near the existing function that does the three replacements (in the same `&`-first order to avoid double-escaping).

2. **Apply the helper to body text** in both the `Ok(text)` and `Err(err)` arms of the match, so the `write!` calls use the escaped versions.

3. **Update the existing test** `compose_reintegration_message_handles_angle_brackets` (line 301): change the assertion from expecting raw `Vec<String>` to expecting the escaped form `Vec&lt;String&gt;` etc.

4. **Add a new test** `compose_reintegration_message_escapes_body_closing_tag` that verifies a body containing `</task>` is escaped to `&lt;/task&gt;` so it doesn't corrupt the XML structure. Similarly test `&` in body text.

5. **Add a new test** `compose_reintegration_message_escapes_error_text` that verifies the error text arm also escapes properly (e.g., an error message containing `<` characters).
