Blocks: Confirmed: `claude -p --input-format stream-json` ignores stdin messages sent mid-stream. The steering VCR test proves this — the steering message "Actually, just count the lines instead" was sent after tool 1, but Claude's thinking says "Simple file. Let me summarize it." and responds with a summary (original task), not a line count. Follow-up messages sent after a result DO work (multi_turn test). Steering needs to be redesigned — likely by interrupting the session and resuming with the steering message as a follow-up.

## Should steering use interrupt-and-resume?

Since stdin messages are ignored mid-stream, the main alternative is to kill the claude process and restart with `--resume` plus the steering message as a new user turn. This would make steering functionally equivalent to: "stop what you're doing, read this message, and continue."

Options:

1. **Interrupt and resume**: Kill the claude process (or close its stdin to trigger graceful shutdown), then spawn a new `claude -p --resume <session-id>` with the steering message. This reuses the existing session context so Claude sees the full conversation history plus the new message.

2. **Convert steering to follow-up only**: Remove mid-stream steering entirely. All user messages are queued as follow-ups (sent after the current turn completes). This is simpler and already works, but loses the "redirect Claude while it's working" use case.

3. **Wait for Claude Code to support mid-stream stdin**: If this is a bug or missing feature in `claude -p`, it might be fixed upstream. We could disable steering for now and re-enable when/if support lands.

The interrupt-and-resume approach is the most capable but also the most complex. It overlaps significantly with the session-interrupt feature (questions/session-interrupt.md) — the interrupt mechanism would be shared.

Answer:

## When is it safe to interrupt?

If we go with interrupt-and-resume, timing matters. Killing claude mid-tool-execution could leave side effects (partially written files, incomplete git operations, etc.).

Options:

1. **Interrupt immediately**: Kill as soon as the user presses Enter. Accept the risk of mid-tool interruption — Claude will see the partial state when it resumes and can recover. This is the simplest and most responsive.

2. **Interrupt at next safe point**: Buffer the steering message and wait for a natural break — either a `content_block_stop` (end of a tool call or text block) or `message_stop` (end of the full turn). Then kill and resume. Safer but less responsive, and if Claude is on a long tool call the delay could be significant.

3. **Interrupt between tool calls only**: Wait for the current tool to finish executing (watch for the tool result in the stream), then kill before Claude starts the next action. This avoids partial tool execution but still allows redirecting between steps.

Answer:

## How should the steering message be sent on resume?

When resuming with `--resume`, we need to include the steering message. Options:

1. **As the initial prompt**: Pass the steering text as the `-p` prompt argument alongside `--resume`. This makes it the next user message in the conversation.

2. **Via stdin after spawn**: Spawn with `--resume` and no prompt, then write the steering message to stdin (the same way follow-ups work after a result). This is how multi_turn already works and is proven to work.

3. **Prefixed with context**: Wrap the steering message with context like "I interrupted your previous response to say: <message>". This helps Claude understand it was a redirect, not a sequential follow-up.

Answer:
