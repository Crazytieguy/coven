Issue: [P2] Migrate from deprecated `serde_yaml` to `serde_yml` or alternative
Status: draft

## Approach

Replace `serde_yaml` with `serde_yaml_ng` — an independent, API-compatible continuation of dtolnay's original library. It aims to be a drop-in replacement and is actively maintained (v0.10, migrating internals to safer libyaml bindings).

### Why `serde_yaml_ng` over `serde_yml`?

`serde_yml` is a fork with known quality issues (AI-generated code with soundness problems, broken docs.rs for months). `serde_yaml_ng` is a clean fork from dtolnay's commit 200950, explicitly targeting API compatibility.

### Changes

1. `cargo add serde_yaml_ng && cargo remove serde_yaml`
2. In `src/dispatch.rs`: change `use serde_yaml::Value` to `use serde_yaml_ng::Value`, and `serde_yaml::from_str` to `serde_yaml_ng::from_str`
3. In `src/agents.rs`: change `serde_yaml::from_str` to `serde_yaml_ng::from_str`
4. Verify: `cargo clippy`, `cargo test`

No API changes, no behavior changes, no test re-recording needed.

## Questions

None — the migration is mechanical.

## Review

