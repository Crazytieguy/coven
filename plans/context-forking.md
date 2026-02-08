Issue: [P2] Big issue: support model-driven context forking. Model outputs xml with definitions for some number of sub tasks (just an assistant message), coven parses it to find the sub tasks and creates a forked claude session per sub task with a simple follow up message like "you own subtask N", waits for all subtasks to complete, and then continues the original session, displaying to the model the final assistant message for each subtask
Status: draft

## Approach

### Overview

When the parent Claude session emits an assistant message containing subtask XML (e.g. `<subtask id="1">...</subtask>`), coven intercepts the Result event, parses the subtasks, spawns a parallel Claude session per subtask (each as a fresh `claude -p` process with the subtask definition as prompt), waits for all children to complete, then sends their results back to the parent session as a follow-up message.

### Phase 1: XML parsing

New module `src/protocol/fork.rs`:

```rust
pub struct Subtask {
    pub id: String,
    pub description: String, // the text content inside the <subtask> tag
}

pub fn extract_subtasks(text: &str) -> Option<Vec<Subtask>>
```

Parse `<subtasks>` block from assistant message text. Return `None` if no subtasks found. Use a simple regex or string-scanning approach (like the existing `scan_break_tag`) rather than pulling in an XML parser — the format is controlled by the system prompt so we can keep it simple.

Example expected format:
```xml
<subtasks>
<subtask id="1">Implement the login form with email validation</subtask>
<subtask id="2">Write unit tests for the auth middleware</subtask>
</subtasks>
```

### Phase 2: Parallel child session spawning

New function in `src/commands/run.rs` (or a new `src/commands/fork.rs` if it gets large):

```rust
async fn run_forked_sessions(
    subtasks: &[Subtask],
    extra_args: &[String],
    renderer: &mut Renderer<impl Write>,
) -> Result<Vec<ChildResult>>
```

For each subtask:
1. Create a `SessionConfig` with `prompt: "You own subtask {id}: {description}"` and the same `extra_args` as the parent.
2. Spawn a `SessionRunner` via the existing `SessionRunner::spawn()`.
3. Run each child through a simplified session loop (no user input — just consume events until Result).

All children run concurrently via `tokio::spawn` + `futures::future::join_all`. Each child gets its own `Renderer` writing to a buffer (not the terminal) so we can capture output without interleaving.

```rust
pub struct ChildResult {
    pub subtask_id: String,
    pub result_text: String,
    pub cost_usd: f64,
    pub success: bool,
}
```

### Phase 3: Result aggregation and parent continuation

After all children complete, format their results and send back to the parent as a follow-up:

```
The forked subtasks have completed. Here are the results:

## Subtask 1: Implement the login form with email validation
<child's final assistant message text>

## Subtask 2: Write unit tests for the auth middleware
<child's final assistant message text>
```

This uses the existing `runner.send_message()` on the parent session, which continues the parent's context. The parent session then sees all subtask results and can continue.

### Phase 4: Display

While children are running, render progress on the terminal:

- When forking starts: `Forking into N subtasks...`
- Per child, a compact status line: `[fork 1/3] Running: Implement the login form...` updated as each completes
- When all complete: `All N subtasks completed ($X.XX total)`
- Then the parent session continues streaming normally

Reuse the existing dim/muted styling from subagent rendering. Don't show full streaming output for children — just status lines. Users can inspect child details via the `:N` message viewer if we store child messages.

### Phase 5: System prompt injection

The parent session needs to know the subtask XML format. Add a `--fork` CLI flag that appends a system prompt section explaining the format:

```
When you need to parallelize work, output subtask definitions in this format:
<subtasks>
<subtask id="1">Description of subtask 1</subtask>
<subtask id="2">Description of subtask 2</subtask>
</subtasks>
Coven will fork into parallel sessions and return results to you.
```

This keeps forking opt-in and doesn't pollute the system prompt when not needed.

### Files to modify/create

| File | Change |
|------|--------|
| **NEW** `src/protocol/fork.rs` | Subtask XML parsing |
| `src/protocol/mod.rs` | Re-export fork module |
| `src/commands/run.rs` | Fork detection after Result, child spawning, result injection |
| `src/commands/session_loop.rs` | Possibly extract a "headless" session loop variant for children |
| `src/display/renderer.rs` | Fork progress display methods |
| `src/cli.rs` | `--fork` flag |
| `src/main.rs` or `src/commands/run.rs` | Pass fork flag through to session setup |

### Incremental delivery

Each phase is independently testable:
1. XML parsing — unit tests
2. Child spawning — integration test with VCR
3. Result aggregation — can test with mock child results
4. Display — snapshot tests
5. System prompt — straightforward CLI plumbing

## Questions

### What XML format should we use for subtask definitions?

The plan assumes a simple `<subtasks><subtask id="N">description</subtask></subtasks>` format. Alternatives:

A. **Minimal (proposed):** Just id + text content. Simple to parse, Claude can easily generate it.
B. **Rich:** Add attributes like `<subtask id="1" name="Login form" priority="high">detailed instructions</subtask>`. More structured but harder to parse and more likely Claude produces malformed output.
C. **JSON in XML:** `<subtasks>[{"id": "1", "prompt": "..."}]</subtasks>`. Easier to parse but uglier for Claude to produce.

Answer:

### Should child sessions share the parent's session history?

A. **Fresh sessions (proposed):** Each child starts blank with just the subtask prompt. Simpler, cheaper (no context replay), but children lack parent context.
B. **Resumed sessions:** Fork from parent's session_id. Children see full parent history. More expensive but children have context. Unclear if Claude's `--resume` supports multiple concurrent resumes from the same session.
C. **Context summary:** Parent's result text + subtask prompt. Middle ground — children get a summary without full history.

Answer:

### How should we handle child failures?

A. **Continue with partial results (proposed):** If a child crashes or errors, report it in the aggregated results and let the parent decide what to do.
B. **Abort all on first failure:** Kill remaining children, report error to parent.
C. **Retry failed children:** Attempt once more before reporting failure.

Answer:

### Should steering/input be supported during fork execution?

A. **No input during fork (proposed):** Disable steering while children run. Simpler, avoids the question of which child receives input.
B. **Broadcast to all children:** Send steering to every child. Unusual but could be useful ("stop and summarize what you have").
C. **Ctrl+C aborts fork:** Allow interrupt to kill all children and return to parent with partial results.

I'd lean toward A + C combined — no steering but Ctrl+C as an escape hatch.

Answer:

## Review

