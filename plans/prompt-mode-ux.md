Issue: When entering prompt mode there should be some indication, and also it should always happen on a new line (i.e. if there's a tool call or assistant message in progress). When the prompt is sent the text should be cleared so that buffered text can be printed in the correct location, and the prompt text should be reprinted when its sent to claude via stdin (but only after the current streaming message completes, if there is one)
Status: draft

## Approach

This issue is about the mid-stream steering input experience. Currently when a user starts typing while Claude is streaming, the characters are echoed inline mixed with Claude's output, with no visual separation. The fix involves four coordinated changes:

### 1. New line + prompt indicator on input activation

When the first character is typed mid-stream, ensure we start on a fresh line and show a `> ` prefix before echoing.

**InputHandler changes** (`src/display/input.rs`):
- Return a new `InputAction::Activated(char)` variant when the first character starts input mode (instead of echoing directly).
- Remove the direct stdout echo from the activation path — let session_loop handle it.

**Renderer changes** (`src/display/renderer.rs`):
- Add `begin_input_line(&mut self)`: closes any open tool line, ends any in-progress text line with `\r\n`, then prints the `> ` prompt prefix. This ensures input always starts on a new line.

**session_loop changes** (`src/commands/session_loop.rs`):
- Handle `InputAction::Activated(c)`: call `renderer.begin_input_line()`, then echo the character.

### 2. Clear prompt line on submit

When the user presses Enter, clear the entire `> text` line from the display instead of just printing `\r\n`.

**InputHandler changes**:
- On Enter, instead of printing `\r\n`, move cursor to start of line and clear it: `\r` + `ClearType::CurrentLine`. Return the text as before via `Submit`.

### 3. Flush buffered events in correct position

This already works — `flush_event_buffer` is called immediately on submit in session_loop. After clearing the prompt line (step 2), the buffered output prints where the prompt was, maintaining correct visual flow.

### 4. Reprint user message after flush

After flushing buffered events, print a styled echo of what the user typed so there's a record in the terminal output.

**Renderer changes**:
- Add `render_user_message(&mut self, text: &str, mode: InputMode)`: prints a styled line like `> text` (for steering) or `>> text` (for follow-up) using `prompt_style`.

**session_loop changes**:
- After `flush_event_buffer` in the `InputAction::Submit` handler, call `renderer.render_user_message(&text, mode)`.

### Files changed

- `src/display/input.rs` — new `Activated(char)` variant, submit line-clearing
- `src/display/renderer.rs` — `begin_input_line()`, `render_user_message()`
- `src/commands/session_loop.rs` — handle `Activated`, call render_user_message after flush

### Edge cases

- **Escape while input active**: Already clears the line. After this change, also needs to clean up the `> ` prefix — the existing Esc handler clears `CurrentLine` which should cover it.
- **Follow-up prompt (WaitingForInput)**: `show_prompt()` already handles this and won't be affected. The `begin_input_line` path is only for mid-stream activation.
- **VCR tests**: No VCR changes needed — VCR tests don't exercise real terminal input. Snapshot tests that replay stdout-only events are unaffected.

## Questions

### Should the `> ` prefix use the existing prompt style or a distinct style?

The existing `prompt_style()` (used in `show_prompt()`) applies styling to `> `. For mid-stream input, we could use the same style for consistency, or a different color/attribute to distinguish steering from follow-up prompts.

Options:
- Same `prompt_style()` for both (consistent, simple)
- Dim/italic for mid-stream steering vs. normal for follow-up (visually distinguishes the two modes)

Answer:

### Should the reprinted user message include a mode indicator?

When the user's message is reprinted after flush, should it indicate whether it was sent as steering vs. follow-up?

Options:
- `> text` for steering, `>> text` for follow-up (lightweight visual distinction)
- Just `> text` for both (simpler)

Answer:

## Review

