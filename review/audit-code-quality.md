---
priority: P1
state: review
---

# Audit codebase for code quality issues

Audit the codebase for any code quality issues, propose changes and refactors.

## Audit Summary

The codebase is well-structured overall: clean module boundaries, consistent error handling (`thiserror` + `anyhow`), strong test coverage via VCR, and clippy passes clean. The issues below are the concrete improvements worth making.

## Plan

### 1. Extract `format_tool_view` and `format_tool_detail` into a dedicated module

**File:** `src/display/renderer.rs` (1209 lines — largest file in the codebase)

`format_tool_view` (lines 775-852) and `format_tool_detail` (lines 856-926) are pure formatting functions with no dependency on `Renderer` state. They share the `get_str` and `first_line` helpers. Extract all four into `src/display/tool_format.rs` and re-export from `renderer.rs` as needed.

This is the single highest-impact refactor: it moves ~180 lines of self-contained logic out of the renderer, making both files easier to navigate. The two functions also have a maintenance coupling (noted by the `NOTE: When adding a tool here, also update [...]` comments) — co-locating them makes that relationship obvious.

### 2. Deduplicate `coven_dir()` / git-common-dir resolution

**Files:** `src/worker_state.rs:31-51`, `src/commands/worker.rs:662-686`

Both `worker_state::coven_dir()` and `worker.rs::resolve_ref_paths()` run `git rev-parse --git-common-dir` and handle the relative-vs-absolute path normalization identically. Extract a shared `git_common_dir(repo_path: &Path) -> Result<PathBuf>` helper into `worktree.rs` (which already owns git operations) and call it from both sites.

### 3. Remove the `InboundEvent` double-boxing in `AppEvent::Claude`

**Files:** `src/event.rs:9`, `src/protocol/types.rs:11`

`AppEvent::Claude(Box<InboundEvent>)` wraps `InboundEvent`, but `InboundEvent::StreamEvent` is already `Box<StreamEvent>`. The outer `Box` was likely added to reduce the `AppEvent` enum size, but `InboundEvent` is already 136 bytes due to the `StreamEvent` variant being boxed — the remaining variants are all small. Measure with `std::mem::size_of::<InboundEvent>()` and if it's reasonable (≤ ~100 bytes after removing the box), remove the `Box<InboundEvent>` wrapper. If it's too large, keep it but add a comment explaining why.

**Note:** This touches many call sites (`Box::new(event)` in `runner.rs:206`, `&*inbound` pattern matches in `session_loop.rs` and `fork.rs`). If the size isn't worth the churn, skip this item.

### 4. Inline single-use VCR wrapper `vcr_resolve_ref_paths`

**File:** `src/commands/worker.rs:449-454`

`vcr_resolve_ref_paths` is called exactly once (line 701). Unlike the other `vcr_*` wrappers which are called from multiple sites or have non-trivial arg construction, this one adds no value as a named function. Inline the VCR call at the use site.

`vcr_main_head_sha` (lines 441-446) is called twice (lines 705 and 713), so it earns its name — leave it.

### 5. Use `write!` instead of `format!` + `push_str` in `compose_reintegration_message`

**File:** `src/fork.rs:187-209`

The function already imports `std::fmt::Write` and uses `write!` for the task entries, but constructs the opening/closing tags with `String::from` and `push_str`. Minor, but using `write!` consistently is cleaner.

## Questions

- For item 3 (removing the `Box<InboundEvent>`): are you comfortable with the churn across call sites, or would you prefer to skip it and just add a comment?
