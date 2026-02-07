Blocks: Probably want a way to interrupt a session before prompting (requires restarting it with --resume, same as if the session ends organically)

## What key triggers the interrupt?

Currently Ctrl+C kills the claude process and exits coven entirely. There are a few options:

1. **Repurpose Ctrl+C** as "interrupt current response" (soft interrupt) and use Ctrl+C again (double-tap) or Ctrl+D as "exit coven entirely." This matches how many REPLs work (Ctrl+C cancels current input/operation, Ctrl+D exits).
2. **New keybinding** like Escape for interrupt, keeping Ctrl+C as hard exit.
3. **Context-dependent Ctrl+C**: while Claude is generating, Ctrl+C interrupts the response; while at a prompt, Ctrl+C exits.

Option 3 seems most intuitive — Ctrl+C means "stop what's happening now." At a prompt, that means exit; during a response, that means interrupt.

Answer:

## After interrupting, what should coven do?

The issue mentions "restarting with --resume, same as if the session ends organically." Options:

1. **Show prompt immediately**: Kill the process, display a status line (e.g. "Interrupted — session abc123"), then show the `> ` prompt. When the user types a message, spawn a new claude process with `--resume` and the session ID.
2. **Show session ID and exit**: Just kill cleanly, print the session ID so the user can manually resume. (Less useful, defeats the purpose.)
3. **Auto-resume and send follow-up**: Kill, immediately restart with `--resume`, and then show the prompt. The new process picks up where the old one left off.

Option 1 seems right — it mirrors the normal "session completed, waiting for follow-up" flow but triggered manually.

Answer:

## Should this work in ralph mode?

In ralph mode, each iteration is a fresh session. If the user interrupts mid-iteration:

1. **End the loop entirely** (current behavior with Ctrl+C)
2. **Skip to next iteration**: Kill the current session, move on to the next ralph iteration with a fresh session
3. **Not applicable**: Ralph mode doesn't support interrupt-and-resume since each iteration is independent

I'd lean toward option 1 (end the loop) since ralph iterations are independent sessions. The interrupt-and-resume feature mainly makes sense for interactive single-session use.

Answer:
