Issue: [P1] Iterate on the fork UI, test case, and system prompt: strict prompting should not be needed, UI should look nicer, the test case should be slightly more interesting
Status: draft

## Approach

Three changes: improve the system prompt so Claude forks without heavy-handed instructions, refine the display, and re-record a better test case.

### 1. System prompt

Current prompt is minimal and works but the test prompt has to say "You MUST emit a `<fork>` tag. Do NOT do these sequentially." A better system prompt should make forking feel like a natural tool rather than an obscure protocol.

**Proposed revision:**

```
When a task has independent subtasks that can run in parallel, emit a <fork> tag with a YAML list of short task labels:
<fork>
- Refactor auth module
- Add tests for user API
</fork>
Each label becomes a separate session that inherits your full context and runs concurrently. You'll see the combined results in a <fork-results> message once all finish. Prefer forking over sequential work when subtasks don't depend on each other.
```

Key changes: opens with *when* to fork (independent subtasks), adds "Prefer forking over sequential work" as a nudge, says "concurrent" not just "parallel".

### 2. Fork UI

Current display:
```
[2] ⑂ Fork  Task A · Task B
  [2/1] ⑂ Write  (1 line)  /path/...
  [2/2] ⑂ Write  (1 line)  /path/...
  ⑂ Task A done
  ⑂ Task B done
```

Issues: the `⑂` character is obscure and may not render on all terminals; "done" is lowercase and abrupt; `[P/C]` numbering is confusing since P is the fork's tool number, not the child index.

**Proposed changes:**

- Replace `⑂` with `⑂` — actually keep the character but consider alternatives. The real problem is readability. Options:
  - `⑂` (current) — obscure but compact
  - `🔀` — emoji, may not match project's no-emoji aesthetic
  - `╠` / `├` — box-drawing, feels like a tree
  - `⑂` but styled bold instead of dim — makes it more visible

- Simplify child numbering: instead of `[2/1]` use `[2a]` or just continue the global tool counter `[3]`, `[4]`. Continuing the global counter is simplest and most consistent with non-fork tool display.

- Change child done line from `⑂ Task A done` to `⑂ Task A ✓` or just `✓ Task A` — more scannable.

- Show task labels on child tool lines for disambiguation: `  [3] ⑂ Task A: Write  (1 line)  /path/...`

### 3. Test case

Replace the current `fork_basic` test with a slightly more realistic scenario. Instead of "create hello.txt and world.txt" (which Claude can easily do sequentially), use a prompt that naturally benefits from parallelism:

**Proposed prompt:** Something like "Read foo.txt and bar.txt and tell me the total line count" where two Read operations are naturally independent. Or "Create a Python hello-world script and a JavaScript hello-world script" — two clearly independent creation tasks.

The prompt should NOT mention fork tags or force forking. If the improved system prompt works, Claude should choose to fork on its own.

Re-record with `cargo run --bin record-vcr fork_basic` after changes.

## Questions

### Should child tool calls continue the global tool counter?

Currently fork children use `[P/C]` numbering (e.g. `[2/1]`, `[2/2]`). Options:

1. **Continue global counter** (`[3]`, `[4]`, ...) — consistent with normal tool display, simple, but loses the visual grouping under the fork
2. **Letter suffix** (`[2a]`, `[2b]`) — preserves grouping, compact
3. **Keep `[P/C]`** — current behavior, explicit but unusual notation

Answer:

### What symbol/style for fork lines?

1. **Keep `⑂` but make it bold cyan** instead of dim cyan — more visible, same character
2. **Switch to `├`/`└`** box-drawing — tree-like structure, very readable
3. **Keep current dim `⑂`** — it's fine, focus changes elsewhere

Answer:

### How to show child completion?

1. **`✓ Task A`** — clean checkmark prefix
2. **`⑂ Task A done`** (current) — explicit but verbose
3. **No completion line** — just let the fork-complete transition speak for itself, less noise

Answer:

## Review

