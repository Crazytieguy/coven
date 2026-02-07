Blocks: I'm unclear on the treatment of thinking: am I seeing streaming thinking, or is thinking not shown? Neither is what I wanted, I just want to see a collapsed thinking indicator (user can display the same way they'd display a tool call). That said, if thinking is currently displayed then I'm really enjoying reading it (out of curiosity), so we should have a cli flag to show all thinking

## Current state

The collapsed indicator is already implemented: thinking blocks show "Thinking..." in dim italic, and the actual thinking text is discarded. So the default behavior matches "collapsed thinking indicator."

Two features remain unclear:

## Should thinking be viewable via :N like tool calls?

The issue says "user can display the same way they'd display a tool call" — this implies thinking content should be stored and inspectable via the `:N` pager command. Is that the intent?

If so, thinking would need to be stored in the message list. The `:N` display would show the full thinking text.

Answer: Correct, that's what I want. Don't discard the thinking. But I notice I'm surprised by the current state: I'm clearly seeing streaming tokens that look like thinking tokens, and I never see "Thinking...". There could be a couple possible hypotheses for this: either these really are thinking tokens and our code currently parses both thinking and non thinking tokens the same (perhaps due to a quirk of claude -p with streaming), or for some reason thinking is disabled and the model (opus 4.6) is using regular tokens as if they were thinking tokens (also due to a quirk of claude -p). Either way: we should investigate the behavior of claude -p and ensure we're parsing correctly and calling claude -p correctly. If there's a claude code bug we should document it somewhere

## What should the CLI flag for showing thinking do?

The issue mentions wanting "a cli flag to show all thinking." Options:

**A) `--show-thinking`: Stream thinking text inline as it arrives**
Full thinking text streams in real time, in a distinct style (e.g. dim italic), similar to how assistant text streams. This replaces the "Thinking..." collapsed indicator with the actual content.

**B) `--show-thinking`: Print thinking text as a block after the thinking block completes**
Wait for the full thinking text, then display it all at once (non-streaming). Simpler but loses the real-time aspect.

**C) Both — `--show-thinking` with optional `--no-stream` interaction**
`--show-thinking` streams by default, `--show-thinking --no-stream` shows it as a completed block.

Answer: Option A
