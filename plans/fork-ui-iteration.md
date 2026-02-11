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
- Write tests for auth module
- Update API documentation
</fork>
Each task inherits your full conversation context and runs as an independent session. Multiple tasks run concurrently. Use fork when a subtask is self-contained — whether you're parallelizing independent work or delegating a task that benefits from full context inheritance. You'll receive the combined results in a <fork-results> message when all tasks complete.
```

### 2. Fork UI (src/display/renderer.rs, src/display/theme.rs)

Each fork task gets its own top-level `[N]` message number, making it viewable with `:N` just like regular tool calls. Child tool calls nest under their task's number.

**Current display:**
```
[2] ⑂ Fork  Task A · Task B
  [2/1] ⑂ Write  (1 line)  /path/...
  [2/2] ⑂ Write  (1 line)  /path/...
  ⑂ Task A done
  ⑂ Task B done
```

**New display:**
```
[2] ⑂ Fork  Task A
[3] ⑂ Fork  Task B
  [2/1] Write  (1 line)  /path/...
  [3/1] Read  /other/...
  [2/2] Write  (3 lines)  /another/...
[4] ⑂ Fork Result
```

Key changes:
- **Each task gets its own `[N]`** — increment `tool_counter` once per task, not once for the whole fork
- **Child tool calls use `[P/C]`** where P is the task's own number — no `⑂` prefix on child lines
- **`[N] ⑂ Fork Result`** line at the end stores the combined result as a StoredMessage for `:N` viewing
- **Bold cyan** instead of dim cyan (theme.rs: change `Attribute::Dim` to `Attribute::Bold`)

Implementation details:

**`ActiveFork` struct** — replace the current `tool_number: usize` + `child_counter: usize` with:
```rust
struct ActiveFork {
    task_numbers: Vec<usize>,    // tool_counter number for each task
    child_counters: Vec<usize>,  // per-task child counter
    task_labels: Vec<String>,    // for reference in completion
}
```

**`render_fork_start(&mut self, tasks: &[String])`** — instead of one line with joined labels:
1. For each task, increment `tool_counter` and render `[N] ⑂ Fork  task_label`
2. Store each task's tool number in `ActiveFork.task_numbers`
3. Each task line gets a StoredMessage with label `[N] ⑂ Fork` — the content should include the task label so `:N` shows what the task was

**`render_fork_child_tool_call(&mut self, task_idx: usize, name: &str, input: &Value)`** — add `task_idx` parameter:
- Use `task_numbers[task_idx]` as parent number P
- Increment `child_counters[task_idx]` for child number C
- Render `  [P/C] ToolName  detail` (no `⑂` prefix on child lines)
- StoredMessage label: `[P/C] ToolName`

**`render_fork_child_done(&mut self, task_idx: usize)`** — no visible output. The task's completion is implicit; its full result text gets stored as the StoredMessage result on the task's `[N]` entry (similar to how `apply_tool_result` works for regular tools).

**`render_fork_complete(&mut self)`** — increment `tool_counter`, render `[N] ⑂ Fork Result`, store a StoredMessage containing the combined fork results. Then clear `self.active_fork`.

**fork.rs `run_fork`** — pass `idx` to renderer calls:
- `render_fork_child_tool_call(idx, name, input)`
- `render_fork_child_done(idx)` — also pass the child's result text so the renderer can store it on the task's StoredMessage
- The combined result text (used for the Fork Result StoredMessage) is the existing `results` vec that gets composed into the `<fork-results>` message

### 3. Test cases

**Replace `fork_basic`** with a more realistic scenario. Proposed prompt: ask for two research tasks (using WebFetch or WebSearch) that each produce a summary. The prompt should NOT mention fork tags — the improved system prompt should make the model fork naturally.

The test fixture's settings.json needs to grant permissions for `WebFetch` and `WebSearch` (no Write needed — `acceptEdits` covers file creation).

This is subject to iteration — the model may not fork naturally on the first try. If the system prompt isn't sufficient, adjust the test prompt to be more suggestive (e.g., emphasize the tasks are independent) rather than making the system prompt more prescriptive. Multiple record-vcr iterations may be needed until the snapshot looks excellent.

**Add `fork_single`** test case demonstrating single-task fork (context-preserving delegation). The prompt should naturally lead the model to delegate one self-contained subtask. Again, iteration until the snapshot looks excellent is expected.

After changes: `cargo run --bin record-vcr fork_basic` and `cargo run --bin record-vcr fork_single`, then `cargo insta accept`.

## Questions

None — previous questions answered by reviewer.

## Review

