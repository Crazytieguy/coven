Blocks: When entering prompt mode there should be some indication, and also it should always happen on a new line (i.e. if there's a tool call or assistant message in progress). When the prompt is sent the text should be cleared so that buffered text can be printed in the correct location, and the prompt text should be reprinted when its sent to claude via stdin (but only after the current streaming message completes, if there is one)

## What visual indication should appear when the user starts typing mid-stream?

Currently, when the user types during streaming output, characters are echoed inline — they get mixed in with Claude's streaming text. The issue says "there should be some indication" when entering prompt mode, and "it should always happen on a new line."

Options for what the new line looks like:

1. **`> ` prompt prefix**: Print `\r\n> ` when the first character is typed, then echo characters after the `> `. This matches the prompt shown in `wait_for_followup` and is familiar.
2. **Dimmed/colored prefix**: Something like `[steering] ` or `[typing...] ` in dim text before the user's input, to distinguish it from Claude's output.
3. **Just a newline**: Move to a new line before echoing, but no prefix. The user's typing is distinguished by being unformatted plain text vs. Claude's styled output.

Option 1 seems most natural since `> ` is already the prompt character. But it could be confusing — the `> ` prompt currently means "session is done, type a follow-up." If it also appears mid-stream for steering, the user might not realize the difference between steering (Enter) and follow-up (Alt+Enter) in that context.

Answer:

## How should the user's message appear after submission?

The issue says: "the text should be cleared so that buffered text can be printed in the correct location, and the prompt text should be reprinted when its sent to claude via stdin."

The "clearing" part is clear — erase the typed text from the display, flush buffered events. But "reprinted" has UX options:

1. **Echo as-is**: Print the user's text on its own line, like `> redirect to writing a poem instead` in plain or dim text, then continue with Claude's output.
2. **Styled user message**: Print something like `You: redirect to writing a poem instead` with a distinct color/style to clearly mark it as user input in the transcript.
3. **Minimal**: Just a dim one-liner like `[sent: redirect to writing a poem instead]` as a confirmation, then continue.

The issue also says "only after the current streaming message completes" — does "completes" mean:
- (a) The current content block (text block, tool use block) finishes?
- (b) The entire assistant turn finishes (all blocks until the next Result event)?
- (c) Just the current text delta burst (i.e., a natural pause in streaming)?

Option (a) seems most likely — wait for the current `content_block_stop` before reprinting, since that's when there's a natural visual break.

Answer:

## Should steering and follow-up messages look different when echoed?

When the user submits text mid-stream, it could be a steering message (Enter, sent immediately to stdin) or a follow-up (Alt+Enter, queued until Result). These are functionally quite different:

- Steering: fire-and-forget, may or may not affect Claude's current turn
- Follow-up: guaranteed to start a new Claude turn after the current one finishes

Should the echo/reprint distinguish between them?

1. **Same display**: Both show as `> message text` (or whatever the reprint style is). The user knows what they pressed.
2. **Different prefix**: e.g., `> message` for steering, `>> message` or `[queued] message` for follow-up. Helps the user confirm which mode was triggered.
3. **Follow-up handled separately**: Don't reprint follow-ups here at all — that's the queued-messages-display issue's territory. Only handle steering reprints in this issue.

Option 3 would keep this issue scoped, but means follow-up messages submitted mid-stream still have no visual feedback until the queued-messages-display issue is addressed.

Answer:
