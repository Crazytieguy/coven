Issue: Subagent rendering issues: incorrect message identifiers (should start from <parent>/1 for each subagent, and should not increment the main-thread message counter). Also parallel agents don't seem to interleave correctly.
Status: draft

## Approach

### Problem

Currently, subagent tool calls use the global `tool_counter` for the child number. If the parent Task is tool #2, the first child Read tool becomes `[2/3]` (because the global counter is at 3). This is confusing — users expect `[2/1]`, `[2/2]`, etc. It also inflates the main-thread counter: the next parent-level tool after a subagent skips numbers (e.g., jumps to `[4]` instead of `[3]`).

Current rendering:
```
[1] Thinking...
[2] ▶ Task  Read and summarize README.md
  [2/3] ▶ Read  /path/to/file
[4] Thinking...
```

Expected rendering:
```
[1] Thinking...
[2] ▶ Task  Read and summarize README.md
  [2/1] ▶ Read  /path/to/file
[3] Thinking...
```

### Changes

**1. Per-subagent counters in `ActiveSubagent`** (`src/display/renderer.rs`)

Add a child counter to `ActiveSubagent`:
```rust
struct ActiveSubagent {
    tool_number: usize,
    child_counter: usize,  // NEW: starts at 0, incremented per child tool call
}
```

Initialize `child_counter: 0` when registering a subagent (around line 515).

**2. Use child counter instead of global counter for subagent tools** (`render_tool_call_line`)

In `render_tool_call_line` (around line 381), when `parent_tool_number` is `Some(p)`:
- Don't increment `self.tool_counter`
- Instead, increment the parent's `child_counter` and use that
- The number label becomes `format!("{p}/{child_n}")`

This requires changing the signature slightly — pass `parent_tool_use_id: Option<&str>` instead of `parent_tool_number: Option<usize>` so we can look up and mutate the `ActiveSubagent` entry.

**3. Adjust `render_subagent_tool_call`** (around line 288)

Currently this calls `render_tool_call_line` with `parent_tool_number`. Change to pass `parent_tool_use_id` so `render_tool_call_line` can manage the child counter directly.

**4. Update `:N` message storage labels**

The `StoredMessage.label` for child tools will now use `[P/C]` numbering (e.g., `[2/1]` instead of `[2/3]`). The `:N` pager command should still work — users type `:2/1` to view the first child tool of task 2.

**5. Update the subagent test snapshot**

Re-record the `subagent` VCR test case and accept the new snapshot. The expected output changes from:
```
[2/3] ▶ Read  ...
[4] Thinking...
```
to:
```
[2/1] ▶ Read  ...
[3] Thinking...
```

### Parallel agents interleaving

Need to investigate this further during implementation. The current HashMap approach should support multiple concurrent subagents, but without a test case it's unclear what "don't interleave correctly" means. Propose:
- Implement the numbering fix first
- Add a parallel-subagent VCR test case to validate interleaving
- Fix any rendering issues that surface

## Questions

### Should `:N` support the slash notation?

Currently `:N` uses the integer message index. With `[2/1]` labeling, should users be able to type `:2/1` to view that specific tool call, or should `:3` still work (where 3 is the sequential position in the messages vector)?

Answer:

### Should we defer the parallel interleaving investigation?

The numbering fix is self-contained and clearly defined. The parallel interleaving issue might need its own test case and could be a separate issue. Should we fix numbering first and split interleaving into a separate issue?

Answer:

## Review

