---
priority: P1
state: review
---

# CDATA injection in fork reintegration message

`src/fork.rs:197-200` wraps fork child results in XML CDATA sections:

```rust
let _ = write!(
    xml,
    "<task label=\"{safe_label}\">\n<![CDATA[{text}]]>\n</task>\n"
);
```

If `text` contains the literal string `]]>`, the CDATA section ends prematurely, producing malformed XML. This breaks the reintegration message sent back to the parent session, potentially causing the model to misinterpret the fork results.

The label is properly XML-escaped (line 190-194), but the body text (which comes from Claude's response) is not CDATA-safe. Claude could easily produce `]]>` in code examples, XML snippets, or tool output.

## Fix

Split `]]>` occurrences in the text into separate CDATA sections: replace `]]>` with `]]]]><![CDATA[>` before wrapping. This is the standard CDATA escaping technique.

The same fix should apply to the error branch (line 202-205).

Add a test case to `fork::tests` with result text containing `]]>` to verify the fix.

## Plan

1. **Add a `cdata_escape` helper** in `src/fork.rs` (private function near `compose_reintegration_message`):
   ```rust
   fn cdata_escape(text: &str) -> String {
       text.replace("]]>", "]]]]><![CDATA[>")
   }
   ```
   This is the standard CDATA escaping technique — it splits the `]]>` sequence across two CDATA sections.

2. **Apply the escape in `compose_reintegration_message`** at lines 197-206. In both the `Ok(text)` and `Err(err)` arms, call `cdata_escape` on the body before interpolating into the CDATA section:
   - `Ok` arm: `<![CDATA[{escaped_text}]]>`
   - `Err` arm: `<![CDATA[{escaped_err}]]>`

3. **Add a test** `compose_reintegration_message_escapes_cdata_end` in `fork::tests`:
   - Input text containing `]]>` (e.g. `"code: <![CDATA[inner]]> end"`)
   - Assert the output does NOT contain the raw `CDATA[inner]]>` sequence adjacent to the outer `]]>`
   - Assert the output contains the escaped form `]]]]><![CDATA[>`
   - Also test the error branch with `]]>` in the error message

4. **Run `cargo fmt`, `cargo clippy`, `cargo test`** to verify.
