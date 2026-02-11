---
priority: P1
state: new
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
