Issue: [P2] Big issue: support model-driven context forking via claude cli's `--fork-session` flag
Status: draft

## Approach

### Overview

When the parent model wants to parallelize work, it emits XML like:

```xml
<fork>
- name: Refactor auth module
  prompt: Focus on extracting the auth middleware into its own module...
- name: Add tests for user API
  prompt: Write integration tests for the /api/users endpoints...
</fork>
```

Coven detects this in the assistant's streamed output, spawns forked child sessions (one per subtask), runs them in parallel, collects their results, and feeds a summary back to the parent session as a follow-up message.

### Detection

- After receiving an `AssistantMessage` or `Result` event, scan the response text for `<fork>...</fork>` (similar to existing `scan_break_tag` in `runner.rs`).
- Parse the YAML list inside the tag. Each entry has `name` (display label) and `prompt` (the follow-up message to send to the child).
- If parsing fails, warn the user and continue the parent session normally (no fork).

### Forking

- For each subtask, spawn a new `SessionRunner` with:
  - `config.resume = Some(parent_session_id)` — resumes from the parent's conversation
  - `config.extra_args` includes `--fork-session` — creates a new session ID instead of mutating the parent
  - `config.prompt = Some(subtask.prompt)` — the subtask-specific follow-up
  - Same `working_dir`, `permission_mode`, `max_thinking_tokens` as parent
- All children share the parent's full context window (that's the key value over native subagents).
- Parent session stays alive but paused (stdin kept open, not killed).

### Parallel execution & display

- Each child session gets its own `mpsc::UnboundedSender<AppEvent>` channel.
- A coordinator task `tokio::spawn`s all children and multiplexes their events onto the main event channel, tagged with a subtask index/name.
- Display options (pick one — see Questions):
  - **Sequential sections**: render each child's output in sequence as they complete, with headers like `── fork: Refactor auth module ──`.
  - **Interleaved with prefixes**: render events as they arrive, prefixed with `[subtask-name]`.

### Result collection & reintegration

- Wait for all children to emit their `Result` event.
- Extract the final response text from each child's result.
- Compose a follow-up message for the parent session:
  ```
  The forked subtasks have completed. Here are the results:

  ## Subtask: Refactor auth module
  <child's final response text>

  ## Subtask: Add tests for user API
  <child's final response text>

  Continue from here.
  ```
- Send this as a follow-up to the parent session via `runner.send_message()`.
- Parent resumes normally.

### Error handling

- If a child session fails (non-zero exit, process crash), include an error note in the summary for that subtask rather than aborting the whole fork.
- If ALL children fail, send a follow-up to the parent explaining the failure so it can retry or take a different approach.
- Timeout: configurable per-fork timeout (default: none). If a child exceeds it, kill it and report partial results.

### Code organization

- New module `src/fork.rs`:
  - `parse_fork_tag(text: &str) -> Option<Vec<Subtask>>` — XML+YAML parsing
  - `run_fork(parent_session_id: &str, subtasks: Vec<Subtask>, config: ForkConfig, event_tx: ...) -> Vec<SubtaskResult>` — orchestrate parallel children
  - `compose_reintegration_message(results: &[SubtaskResult]) -> String`
- Modify `src/commands/session_loop.rs` to check for fork tags after each assistant turn and trigger the fork flow.
- Modify `src/session/runner.rs`: `SessionConfig` already supports `resume` and `extra_args`, so `--fork-session` just goes in `extra_args`. No structural changes needed.

### Integration with ralph mode and worker mode

- In ralph mode: fork results feed back into the current iteration's session. The break-tag scan happens on the parent's final response after reintegration, not on intermediate fork output.
- In worker mode: forks happen within the agent phase. Each child inherits the worker's worktree. Conflict risk is higher since children may edit overlapping files — but that's the model's responsibility to coordinate via the subtask prompts.

## Questions

### XML tag name and schema
Should the fork tag be `<fork>` or something more specific like `<coven-fork>`? A more specific name reduces false positives from the model mentioning fork concepts in prose. The content schema (YAML list with name/prompt fields) seems right but open to alternatives.

Answer:

### Display strategy for parallel children
Two options:
1. **Sequential sections** — buffer all output per child, render completed children one at a time with section headers. Simpler, cleaner, but you don't see progress until each child finishes.
2. **Interleaved with prefixes** — render events as they arrive, prefixed with child name. More responsive but noisier. Could get confusing with many children.
3. **Collapsed progress + expand on completion** — show a single progress line per child (like tool-call lines today), expand full output only after completion or on `:N` inspection.

Option 3 seems most consistent with coven's existing design (one line per tool call, details on demand). But it's the most work.

Answer:

### Working directory isolation
Should forked children share the parent's working directory, or should each get its own git worktree (like orchestration workers)? Shared is simpler and matches the issue description. Worktrees add safety but complexity and wouldn't work well for non-git projects.

Answer:

### Nesting
Should forks be allowed to nest (a child emitting its own `<fork>` tag)? Suggest disabling for v1 — add a `fork_depth` counter and ignore fork tags when depth > 0.

Answer:

## Review

