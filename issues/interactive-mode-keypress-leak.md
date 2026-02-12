---
priority: P1
state: new
---

# Interactive mode (Ctrl+o) leaks keypresses to coven

When entering interactive mode via Ctrl+o, roughly half of keypresses are being sent to coven instead of the child Claude Code instance. Claude Code should fully take over the terminal until it exits — all keypresses should make it to Claude Code with none intercepted by coven.
