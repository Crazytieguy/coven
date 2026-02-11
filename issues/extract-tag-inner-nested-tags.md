---
priority: P1
state: new
---

# `extract_tag_inner` silently truncates nested tags

`src/protocol/parse.rs:8-15` — `extract_tag_inner()` finds the **first** closing tag after the opening tag, which means nested tags of the same name produce wrong results:

```
Input:  "<break>outer <break>inner</break> after</break>"
Actual: Some("outer <break>inner")
Expected: Some("outer <break>inner</break> after")
```

This affects any tag parsed by this function: `<break>` (ralph loop), `<fork>` (fork parsing), and `<dispatch>` (dispatch). Claude's output can easily contain XML-like tags in code examples or explanations.

**Impact:** Ralph's break detection could fire on a partial match if Claude's response contains nested `<break>` tags in code blocks. Fork task parsing could silently truncate task lists.

**Fix options:**
1. Match outermost pair by counting open/close occurrences (proper nesting)
2. Document the limitation and use the **last** closing tag instead of the first (simpler, handles most cases)
3. Keep current behavior but add a comment and test documenting the constraint

Existing tests only cover single-level tags — no test for nested same-name tags.
