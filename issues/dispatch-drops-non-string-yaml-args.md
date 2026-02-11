---
priority: P2
state: new
---

# `dispatch::parse_decision` silently drops non-string YAML arg values

In `src/dispatch.rs:50-61`, when parsing dispatch decision arguments, non-string YAML values are silently filtered out via `v.as_str()?` inside a `filter_map`.

If the dispatch agent outputs something like:

```yaml
<dispatch>
agent: implement
issue: issues/fix-bug.md
priority: 1
verbose: true
</dispatch>
```

The integer `1` and boolean `true` values are silently dropped — only `issue` survives as an argument. The agent template then renders without `priority` or `verbose`, which could affect behavior without any warning.

**Fix options:**
1. Convert non-string values to strings (e.g. `v.as_str().map(String::from).or_else(|| Some(format!("{v}")))`)
2. Warn or error on non-string values so the dispatch agent learns to use strings
3. Accept this as expected behavior and document it in the dispatch output format instructions
