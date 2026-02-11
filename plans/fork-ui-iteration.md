Issue: [P1] Iterate on the fork UI, test case, and system prompt: strict prompting should not be needed, UI should look nicer, the test case should be slightly more interesting
Status: draft

## Approach

Three changes: revise the system prompt philosophy, redesign the fork display, and re-record better test cases.

### 1. System prompt (src/fork.rs `fork_system_prompt`)

Shift from "parallelism tool" to "self-contained subtask delegation". Key changes:

- Lead with the concept: fork lets you delegate self-contained subtasks that inherit your full context
- Single-task forks are valid (for context-preserving delegation)
- Multiple tasks run in parallel — mention this as a benefit, not the sole purpose
- Keep the YAML tag syntax unchanged

Proposed wording:

```
To delegate self-contained subtasks, emit a <fork> tag with a YAML list of short task labels:
<fork>
- Refactor auth module
- Add tests for user API
</fork>
Each task inherits your full conversation context and runs as an independent session. Multiple tasks run concurrently. Use fork when a subtask is self-contained — whether you're parallelizing independent work or delegating a task that benefits from full context inheritance. You'll receive the combined results in a <fork-results> message when all tasks complete.
```

### 2. Fork UI (src/display/renderer.rs, src/display/theme.rs)

**Current display:**
```
[2] ⑂ Fork  Task A · Task B
  [2/1] ⑂ Write  (1 line)  /path/...
  [2/2] ⑂ Write  (1 line)  /path/...
  ⑂ Task A done
  ⑂ Task B done
```

**Proposed display:**
```
[2] ⑂ Fork
    [2/1] Task A
    [2/2] Task B
  [2/1] Write  (1 line)  /path/...
  [2/2] Read  /other/...
  [2/1] Write  (3 lines)  /another/...
  [2/1] ✓ Task A  (:3)
  [2/2] ✓ Task B  (:4)
```

Key changes:
- **Task labels on separate lines**, each prefixed with `[P/C]` where C = task index (1-based)
- **`[P/C]` prefix on every child line** identifies which task owns it — critical since events interleave from the merged channel
- **No `⑂` on child lines** — only on the fork header
- **Bold cyan** instead of dim cyan (theme.rs: change `Attribute::Dim` to `Attribute::Bold`)
- **Completion lines** show `✓ Task Label` with a message reference hint `(:N)` so the user knows how to view the full response

Implementation details:

**`ActiveFork` struct** — currently tracks `tool_number` and `child_counter`. Change to track `tool_number` and `task_labels: Vec<String>` (for referencing in completion lines). Remove `child_counter` since child lines use task index, not a global counter.

**`render_fork_start(&mut self, tasks: &[String])`** — change from joining labels with `·` on one line to:
1. Render `[N] ⑂ Fork` on first line
2. Render `    [N/C] label` for each task on subsequent lines (indented to align with "Fork")

**`render_fork_child_tool_call(&mut self, task_idx: usize, name: &str, input: &Value)`** — add `task_idx` parameter. Use `[P/task_idx+1]` prefix instead of incrementing child_counter. StoredMessage label becomes `[P/C] ToolName`.

**`render_fork_child_done(&mut self, task_idx: usize, label: &str)`** — add `task_idx` parameter. Render `  [P/C] ✓ label  (:M)` where M is the message number. To enable this, store the fork child's full result text as a StoredMessage when the child completes, and reference its index.

**fork.rs `run_fork`** — pass `idx` to renderer calls:
- `render_fork_child_tool_call(idx, name, input)` (currently doesn't pass idx)
- `render_fork_child_done(idx, &tasks[idx])` (currently doesn't pass idx)
- After child completes, push a StoredMessage with the child's result text so it's viewable via `:N`

### 3. Test cases

**Replace `fork_basic`** with a more realistic scenario where each task does meaningful work. Proposed: two web research tasks (using WebFetch or WebSearch) that each write results to a file. The prompt should NOT mention fork tags — the improved system prompt should make the model fork naturally.

Example prompt: "I need two research summaries written to files: 1) find out what Rust's async runtime Tokio is and write a summary to tokio-summary.txt, 2) find out what the Axum web framework is and write a summary to axum-summary.txt"

The test fixture's settings.json needs to grant permissions for web access and file writing.

**Add `fork_single`** test case demonstrating single-task fork (context-preserving delegation). Prompt should naturally lead the model to delegate one self-contained subtask.

After changes: `cargo run --bin record-vcr fork_basic` and `cargo run --bin record-vcr fork_single`, then `cargo insta accept`.

## Questions

### How should the `:N` message reference work for fork child responses?

When a fork child completes, we need to store its full result text as a `StoredMessage` so the user can view it. Two options:

1. **Store at child completion** — when `render_fork_child_done` is called, also push a StoredMessage containing the child's result text. The completion line shows `(:M)` where M is `messages.len()`. Query `:M` retrieves it.
2. **Store as a property of the fork** — store results in the `ActiveFork` struct and only push them when the fork completes. This groups them but delays message availability.

I lean toward option 1 since it's simpler and makes messages available immediately.

The question is: what content goes in the StoredMessage? The child's result is XML text that gets composed into the reintegration message. Should we store the raw result, or a cleaned-up version?

Answer:

### What permissions should the fork test fixtures grant?

The test cases will use web access (WebFetch/WebSearch) and file writing. The settings.json for the test fixture needs appropriate permissions. Options:

1. **Minimal**: Allow only `WebFetch(*)`, `WebSearch(*)`, and `Write(*)` — enough for the scenario
2. **Standard test permissions**: Match whatever the existing test fixtures use

Answer:

## Review

