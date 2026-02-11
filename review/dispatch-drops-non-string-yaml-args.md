---
priority: P2
state: review
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

## Plan

Use **option 1** — convert non-string YAML scalars to strings. This is the most robust approach since all downstream consumers (`HashMap<String, String>`, Handlebars templates, worker state) already treat args as strings. Converting at the parse boundary means the dispatch agent doesn't need to worry about YAML type coercion.

### Changes

**`src/dispatch.rs`** — one change in `parse_decision`, lines 50-60:

Replace the `v.as_str()?` filter in the args collection with a helper that converts any YAML scalar to a string:

```rust
fn yaml_scalar_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Null | Value::Sequence(_) | Value::Mapping(_) | Value::Tagged(_) => None,
    }
}
```

Then the args collection becomes:
```rust
let args = map
    .iter()
    .filter_map(|(k, v)| {
        let key = k.as_str()?;
        if key == "agent" {
            return None;
        }
        let val = yaml_scalar_to_string(v)?;
        Some((key.to_string(), val))
    })
    .collect();
```

Non-scalar values (sequences, mappings, tagged, null) are still silently dropped, which is fine — those would be malformed dispatch output and the required-arg validation in `AgentDef::render` will catch missing required args.

**`src/dispatch.rs` tests** — add one test:

```rust
#[test]
fn parse_non_string_args_converted() {
    let text = "<dispatch>\nagent: implement\nissue: issues/fix-bug.md\npriority: 1\nverbose: true\n</dispatch>";
    let decision = parse_decision(text).unwrap();
    assert_eq!(
        decision,
        DispatchDecision::RunAgent {
            agent: "implement".into(),
            args: HashMap::from([
                ("issue".into(), "issues/fix-bug.md".into()),
                ("priority".into(), "1".into()),
                ("verbose".into(), "true".into()),
            ]),
        }
    );
}
```
