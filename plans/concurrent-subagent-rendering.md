Issue: [P2] Concurrent subagent rendering: support displaying multiple concurrent claude Task subagents running in parallel
Status: draft

## Approach

### Current state

Today, subagent rendering is tracked with a single `last_tool_is_subagent: bool` in `Renderer`. Child tool calls get a 2-space indent prefix and dimmer styling. There's no tracking of *which* subagent a child event belongs to — the boolean simply says "the last tool call was a subagent." This works for a single sequential subagent but breaks with concurrent ones.

### Problem with concurrency

When Claude spawns multiple Task tools in a single assistant message (parallel tool calls), child events from different subagents arrive interleaved. We need to:
1. Associate each child event with its parent subagent
2. Visually distinguish output from different subagents
3. Handle subagents completing in any order

### Proposed changes

#### 1. Track active subagents by ID

Replace `last_tool_is_subagent: bool` with a map of active subagents in `Renderer`:

```rust
struct ActiveSubagent {
    tool_number: usize,     // The [N] counter when this subagent was rendered
    description: String,    // The short description from the Task tool input
}

// In Renderer:
active_subagents: HashMap<String, ActiveSubagent>,  // keyed by tool_use_id
```

When a Task tool call arrives (detected in `handle_inbound`), register it in `active_subagents` with its `tool_use_id`. When the final `UserToolResult` with no `parent_tool_use_id` arrives for that tool, remove it.

#### 2. Thread child events through parent lookup

In `handle_inbound`, when a child event has `parent_tool_use_id`, look up the parent in `active_subagents` to get context (tool number, description). Pass this context to the render methods.

#### 3. Rendering approach

Two reasonable display approaches — see Questions below. Both share the same data model; only the rendering differs.

**Option A: Grouped by subagent (virtual sections)**

Each subagent gets a labeled section. Child tool calls are indented under their parent. When events interleave, output goes to the appropriate section. This requires buffering or rewriting terminal lines.

```
[1] > Task  "Summarize README"
  [2] > Read  README.md
  README describes testing subagent display...
[3] > Task  "Check dependencies"
  [4] > Read  Cargo.toml
  Found 12 dependencies...
```

**Option B: Inline with parent tag (simpler)**

All tool calls render in arrival order. Child calls are prefixed with a short parent identifier (the subagent's [N] number) so the user can visually group them.

```
[1] > Task  "Summarize README"
[2] > Task  "Check dependencies"
  [1:3] > Read  README.md
  [2:4] > Read  Cargo.toml
  [1] README describes testing subagent display...
  [2] Found 12 dependencies...
```

#### 4. Changes by file

- `src/display/renderer.rs`:
  - Replace `last_tool_is_subagent: bool` with `active_subagents: HashMap<String, ActiveSubagent>`
  - Update `render_subagent_tool_call` to register the subagent
  - Update `render_subagent_tool_result` to look up parent context
  - Update `tool_indent` to use active subagent context instead of bool
  - Update tool numbering for child calls (either `[N:M]` or plain `[M]` depending on chosen approach)

- `src/lib.rs`:
  - Pass `tool_use_id` from assistant messages to `render_subagent_tool_call` so it can register the subagent
  - Pass `parent_tool_use_id` to `render_subagent_tool_result` for parent lookup

- `src/session/events.rs` (if needed):
  - Ensure `tool_use_id` is available on assistant content blocks (it likely already is from the JSON)

- Tests:
  - New VCR fixture with concurrent subagents (two parallel Task calls)
  - Verify interleaved events render correctly

## Questions

### Which rendering approach: grouped sections (A) or inline with parent tags (B)?

**Option A (grouped sections):** Cleaner visual grouping — each subagent's activity is contiguous. But harder to implement: requires either buffering events and rendering at completion, or terminal manipulation to insert lines into the right section. May also hide the true chronological order of events.

**Option B (inline with parent tags):** Simpler — events render in arrival order with a parent identifier prefix. Preserves chronological ordering. Easier to implement (no buffering or terminal rewriting). But interleaved output from many subagents could be hard to follow.

I'd lean toward **Option B** for a first pass — it's significantly simpler and preserves the streaming property. Option A could be added later if the display becomes confusing with many concurrent subagents.

Answer:

### Should child tool numbering be scoped per-subagent or global?

**Global (current):** All tool calls share one counter. Simple, consistent with current behavior. But `[7]` gives no hint which subagent it belongs to.

**Prefixed (`[1:3]`):** Parent number + child number. Clearly associates child with parent. But changes the numbering format and affects the `:N` pager command (would need to handle `1:3` syntax).

**Scoped but shown as `[1.3]` or `[1/3]`):** Variations on prefixed. Same tradeoffs.

I'd lean toward global numbering (keeping it simple) plus the 2-space indent + dimmer styling already used. The parent association comes from the indent level and any parent tag shown.

Answer:

## Review

