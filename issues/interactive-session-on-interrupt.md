---
priority: P1
state: new
---

# Keybinding to open interactive session from interrupted state

After interrupting a coven session (Ctrl+C), the user should be able to press a key to drop into the native Claude Code TUI (`claude --resume <session_id>`, no `-p`), continuing the same conversation interactively. When the user exits the native TUI, they return to coven's interrupted state where they can resume non-interactively with a prompt or exit.

No need to clear the display — whatever the interactive session left on screen is fine.

Should work from the interrupted state in run, ralph, and worker.