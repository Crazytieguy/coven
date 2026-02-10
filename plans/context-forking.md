Issue: [P2] Big issue: support model-driven context forking via claude cli's `--fork-session` flag
Status: draft

## Approach

### Overview

When the parent model wants to parallelize work, it emits XML like:

```xml
<fork>
- Refactor auth module
- Add tests for user API
</fork>
```

Each item is a short task label. The model's full context (thinking + conversation) is already available to each fork via `--fork-session`, so no additional prompt field is needed — each child just receives a follow-up like "You were assigned 'Refactor auth module'".

Coven detects this tag in the assistant's streamed output, spawns forked child sessions (one per task), runs them in parallel, collects their results, and feeds a summary back to the parent session as a follow-up message.

### System prompt guidance

Inject fork instructions into `append_system_prompt` so the model knows the `<fork>` tag is available. Something like:

```
To parallelize work, emit a <fork> tag containing a YAML list of short task labels:
<fork>
- Refactor auth module
- Add tests for user API
</fork>
Each fork inherits your full context and runs in parallel.
```

This is the sole mechanism for teaching the model to fork — test fixtures should not prompt the model to use any specific syntax.

### Detection

- After receiving a `Result` event, scan the final response text for `<fork>...</fork>` using the same pattern as `scan_break_tag`.
- Parse the YAML list inside the tag. Each entry is a plain string (task label).
- If parsing fails, send a follow-up message to the parent session explaining the parse error so it can retry with corrected syntax. Continue the session normally (no fork).

### Forking

- For each task label, spawn a new `SessionRunner` with:
  - `config.resume = Some(parent_session_id)` — resumes from the parent's conversation
  - `config.extra_args` includes `--fork-session` — creates a new session ID instead of mutating the parent
  - `config.prompt = Some("You were assigned '<task label>'")` — the follow-up
  - Same `working_dir`, `permission_mode`, `max_thinking_tokens` as parent
- All children share the parent's full context window (that's the key value over native subagents).
- All children share the parent's working directory (no worktree isolation).
- Parent session stays alive but paused (stdin kept open, not killed).
- Nesting disabled for v1: ignore `<fork>` tags in child output.

### Parallel execution & display

Model the display after the existing native subagent rendering, but make it visually distinct:

- **Subagent rendering (existing)**: `[P/C]` numbering, 2-space indent, dimmed yellow tool names.
- **Fork rendering (new)**: Use a similar hierarchical numbering scheme but with a distinct visual marker. For example, render a parent-level line when the fork starts (like `[N] ⑂ Fork  2 tasks`), then render each child's tool calls indented beneath it with `[N/C]` numbering — mirroring the subagent pattern.
- To make forks visually distinct from native subagents, use a different icon/prefix (e.g., `⑂` instead of `▶`) and/or a different color (e.g., cyan instead of yellow).
- Each child session's events are multiplexed onto the main event channel, tagged with a fork index. The renderer tracks active forks in a structure similar to `active_subagents`.

### Result collection & reintegration

- Wait for all children to complete (emit their `Result` event).
- Extract the final response text from each child's result.
- Compose a follow-up message for the parent session using XML tags (not markdown, since assistant messages often contain markdown):
  ```xml
  <fork-results>
  <task label="Refactor auth module">
  <child's final response text>
  </task>
  <task label="Add tests for user API">
  <child's final response text>
  </task>
  </fork-results>
  ```
- Send this as a follow-up to the parent session via `runner.send_message()`.
- Parent resumes normally.

### Error handling

- If a child session fails (non-zero exit, process crash), include an error note in the `<task>` block for that subtask rather than aborting the whole fork.
- If ALL children fail, send a follow-up to the parent explaining the failure so it can retry or take a different approach.
- No timeout mechanism for v1.

### Code organization

- New module `src/fork.rs`:
  - `parse_fork_tag(text: &str) -> Option<Vec<String>>` — extract task labels from `<fork>` tag
  - `run_fork(parent_session_id: &str, tasks: Vec<String>, config: ForkConfig, event_tx: ...) -> Vec<TaskResult>` — orchestrate parallel children
  - `compose_reintegration_message(results: &[TaskResult]) -> String` — XML-formatted results
- Modify `src/commands/session_loop.rs` to check for fork tags after each assistant turn and trigger the fork flow.
- Modify `src/display/renderer.rs`: add fork tracking (similar to `active_subagents`) and fork-specific rendering with distinct visual markers.
- Modify `src/session/runner.rs`: `SessionConfig` already supports `resume` and `extra_args`, so `--fork-session` just goes in `extra_args`. No structural changes needed.

### Mode behavior

Forking works identically in all modes (run, ralph, worker):

- The fork flow triggers whenever a `<fork>` tag is detected in the assistant's final response.
- In ralph mode, the break-tag scan happens on the parent's final response after reintegration, not on intermediate fork output.
- In worker mode, children share the worker's worktree — file conflict risk is the model's responsibility to manage via task descriptions.

## Questions

### Term for fork entries

Each entry in the `<fork>` tag is currently called a "task label". Is there a better word than "task" or "name" for what each fork entry represents? Options: "task", "branch", "strand", "objective". Using "task" for now as it's the most straightforward.

Answer:

## Review

