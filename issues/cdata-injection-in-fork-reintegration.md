---
priority: P1
state: approved
---

# Remove CDATA wrapping from fork reintegration message

`src/fork.rs:197-200` wraps fork child results in XML CDATA sections. If `text` contains `]]>`, the CDATA section breaks. But the real fix is simpler: just drop CDATA entirely.

These XML messages are read by Claude, not parsed by an XML parser. Claude can tell structure from content by context — structured returns don't need to be unambiguous. CDATA adds complexity for no benefit.

## Plan

1. **Remove CDATA wrapping in `compose_reintegration_message`** (`src/fork.rs` lines 197-206). In both the `Ok` and `Err` arms, replace `<![CDATA[{text}]]>` with just `{text}`:
   - `Ok` arm: `"<task label=\"{safe_label}\">\n{text}\n</task>\n"`
   - `Err` arm: `"<task label=\"{safe_label}\" error=\"true\">\n{err}\n</task>\n"`

2. **Run `cargo fmt`, `cargo clippy`, `cargo test`** to verify.
