---
priority: P0
state: approved
---

# Move worker_status from template injection to system prompt

## Problem

`worker_status` is handled as a declared agent arg in `dispatch.md` that gets auto-injected by special-case code in `worker.rs`. This creates several issues:

1. **Bug: missing status on re-dispatch** — Agents (land, implement, plan) are instructed to hand off back to dispatch. When dispatch runs mid-chain, `is_entry` is false, so the injection is skipped and `{{worker_status}}` renders empty. This is the normal flow, not an edge case.

2. **Leaky abstraction** — `worker.rs` has a hardcoded `"worker_status"` string check coupling the worker loop to a specific template convention. Coven should provide orchestration primitives, not reach into agent templates.

3. **Misleading transition protocol** — The `worker_status` arg appears in the transition system prompt examples shown to all agents, but no agent should ever pass it.

## Plan

**Approach:** Move worker status out of agent templates and into the system prompt. Always compute and append worker status to the `--append-system-prompt` content for every agent session. This is simple, generic, and avoids all special-case logic. Claude Code's system prompt already has dynamic content (git status) near the end, so cache impact is negligible.

### Changes

1. **`dispatch.md`** — Remove the `worker_status` arg from frontmatter. Remove the `{{worker_status}}` template variable and the "Current Worker Status" section. No replacement needed — the info now comes via system prompt.

2. **`worker.rs` (~lines 284-302)** — Delete the entire auto-injection block. Instead, after building the system prompt (~line 312-316), always compute worker status and append it:
   ```
   let worker_status = /* read and format */;
   system_prompt.push_str(&format!("\n\n## Worker Status\n\n{worker_status}"));
   ```

3. **`worker_state.rs`** — Delete `format_status` (its only caller was the injection code). Keep `format_workers` and both `StatusStyle` variants (used by `coven status` CLI and now the system prompt injection).

4. **Update snapshot tests** — The transition system prompt snapshots will change because dispatch no longer declares `worker_status` as an arg, and all agent system prompts will now include worker status. Re-record VCR fixtures and accept snapshot diffs.
