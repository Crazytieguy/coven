---
priority: P1
state: review
---

# Evaluate whether worker_status is a good abstraction

Evaluate whether `worker_status` is a good abstraction or if it's needed compared to just using the regular args.

## Analysis

`worker_status` is currently handled as a **declared agent arg** in `dispatch.md` that gets **auto-injected** by special-case code in `worker.rs`. This creates several issues:

1. **Misleading transition protocol** — Since dispatch declares `worker_status` as a required arg, the transition system prompt shown to all agents includes it in the dispatch examples: `worker_status: <What other workers are currently doing>`. No agent should ever pass this — it's auto-computed. This creates noise and could confuse models.

2. **Hardcoded arg name check** — `worker.rs:284-302` checks for the string `"worker_status"` by name, coupling the worker loop to a specific arg name convention.

3. **Neither fish nor fowl** — `worker_status` isn't a real transition arg (no agent passes it via `<next>`), and it isn't a system-level injection either. It occupies an awkward middle ground.

The current approach is _functional_ but the abstraction is leaky. The fix is small.

## Plan

**Approach:** Make `worker_status` a built-in template variable that the worker always injects for the entry agent, rather than a declared agent arg.

### Changes

1. **`dispatch.md`** — Remove the `worker_status` arg from frontmatter. Keep `{{worker_status}}` in the template body (Handlebars non-strict mode renders undeclared variables fine).

2. **`worker.rs` (~line 284-302)** — Replace the `if is_entry && agent_def.frontmatter.args.iter().any(...)` check with just `if is_entry`. Always compute and inject `worker_status` for the entry agent, unconditionally. Remove the comment about "auto-inject if the agent declares it".

3. **Update snapshot tests** — The transition system prompt snapshots will change because dispatch no longer declares `worker_status` as an arg. Run `cargo insta accept` after re-recording if applicable.

This removes the special-case arg name check, stops `worker_status` from polluting the transition protocol, and keeps the template variable available for any entry agent that wants it.

## Questions

- Is there value in making the injection conditional (only compute if the template contains `{{worker_status}}`)? The cost of always reading worker state files is negligible, so I lean toward unconditional injection for simplicity.
