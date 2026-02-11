---
priority: P1
state: review
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

## Plan

Use **fix option 1**: match the outermost pair by counting open/close occurrences. This is the correct fix — option 2 (last closing tag) would break when a response contains two separate same-name tag pairs (e.g. `<break>a</break> text <break>b</break>`), and option 3 doesn't fix anything.

### Changes

**`src/protocol/parse.rs:8-15` — Rewrite `extract_tag_inner` to be nesting-aware:**

Replace the current `find(&close)` with a scan that tracks nesting depth:
- Start after the opening `<tag>` with `depth = 1`
- Scan forward through the remaining text
- On each `<tag>`, increment depth
- On each `</tag>`, decrement depth
- When depth reaches 0, that's the matching closing tag — return the slice between the opening tag and this position

The scan should search for the next occurrence of either `<tag>` or `</tag>` at each step (whichever comes first), not character-by-character. This keeps the logic simple and efficient.

**`src/protocol/parse.rs` tests — Add test cases:**

1. `extract_tag_nested` — nested same-name tags return the full outer content:
   - Input: `"<break>outer <break>inner</break> after</break>"`
   - Expected: `Some("outer <break>inner</break> after")`

2. `extract_tag_two_separate_pairs` — two separate pairs returns the first one (existing behavior preserved):
   - Input: `"<foo>first</foo> gap <foo>second</foo>"`
   - Expected: `Some("first")`

3. `extract_tag_deeply_nested` — multiple nesting levels:
   - Input: `"<t><t><t>deep</t></t></t>"`
   - Expected: `Some("<t><t>deep</t></t>")`
