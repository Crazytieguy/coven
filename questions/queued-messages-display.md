Blocks: When there are queued messages, we should display them somehow bellow the messages that are streaming in. Different display for follow up and steering messages. Not sure what the right technical approach is here.

## Which message types need a queue indicator?

Currently there are two outgoing message types:
- **Steering messages** (Enter): Sent immediately to Claude's stdin. The user sees what they typed echoed, but there's no confirmation it was received or is being processed.
- **Follow-up messages** (Alt+Enter): If Claude is still running, stored in `pending_followup` and sent automatically when a Result event arrives.

Options:
1. **Only follow-ups**: Steering messages are fire-and-forget (they go to stdin immediately), so only follow-up messages need a "queued" indicator since there's a delay before they're sent.
2. **Both types**: Show steering messages briefly as "sent" confirmation (like a toast), and follow-ups as "queued" until they actually get sent. This gives the user confidence that both types were received.
3. **Both, but differently**: Steering gets a brief inline echo (e.g., `> your message` in dim text), follow-ups get a persistent "queued" badge that stays visible until the message is actually sent to Claude.

Answer:

## Where should queued messages appear?

The terminal currently has streaming output flowing downward with no fixed regions. Options:

1. **Inline below output**: Print the queued message indicator directly below the current streaming output, in the normal flow. When Claude's output continues, it pushes the indicator down. When the message is actually sent, the indicator is replaced with the normal flow. Problem: Claude's streaming output would overwrite or interleave with the indicator.
2. **After result, before prompt**: Only show the queued follow-up when Claude's current turn ends and the result line is printed, but before showing the `> ` prompt. At that point, show something like `[queued] your message` then immediately send it. This is simpler but means the user has no visual feedback while Claude is still streaming.
3. **Fixed bottom region (scroll region)**: Reserve the bottom N lines of the terminal for a status area showing queued messages, using terminal scroll regions so streaming output scrolls above. This is the most polished approach but also the most complex (the README already lists "fixed bottom prompt" as a planned feature).
4. **Redraw on the current line**: Use terminal escape codes to write the queue indicator on the line below current output, then clear/rewrite it as output continues. Similar to how progress bars work. This is fragile but avoids scroll regions.

Answer:

## Should multiple follow-up messages be queueable?

Currently `pending_followup` is `Option<String>` — only one follow-up can be queued at a time. If the user sends a second follow-up while one is already queued, it would overwrite the first.

1. **Single queue (current)**: Keep `Option<String>`. If the user queues another, it replaces the previous one. Simple, and in practice users rarely need to queue multiple messages.
2. **FIFO queue**: Change to `VecDeque<String>`. Each follow-up is queued in order and sent one at a time after each Result. More powerful but adds complexity — each queued message would trigger a new Claude turn, and the user might queue messages that no longer make sense by the time they're sent.
3. **Concatenate**: If a follow-up is already queued, append the new text to it (with a newline). The combined message is sent as one prompt. Simple and avoids the multi-turn complexity.

Answer:
