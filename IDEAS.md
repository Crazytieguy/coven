# coven

An oven for claude.

## Use cases

### Minimal streaming display

Run claude with `-p` and streaming json output, parse the json output, and print a minimal (configurable with a config file or cli arguments) display. One line per tool call or thinking, clear indication of rejected tool calls, assistant messages displayed in full, nice rendering.

Communicate with claude via claude code hooks: the cli lets you enter a message, it gets saved to a file, and next time the hook runs the message is displayed to claude and the file is evicted.

If the claude session ends, let the user follow up and use `claude --continue` to continue the session with the new user message (but also let the user know how to continue the session within the actual interactive claude cli).

### Ralph wiggum looping

Same thing, but in a loop. Run claude with the same prompt over and over, but state accumulates in the file system.

### Plan interactively, execute non-interactively

Plan with the native interactive UI and plan mode, but then execute the plan in non-interactive mode. Ideal workflow but unclear exactly how it would be implemented.

### Session inspector

Inspect running or finished sessions interactively to view full details of all tool calls etc. The claude tui by default is a bit bad for this.

## Why `-p` mode?

- **Performance**: each claude tui instance is pretty expensive, and when running several in parallel the computer starts slowing down
- **Permissions**: when claude doesn't have permission to do something, it can keep going and try something else instead of blocking on user input
- **Flexibility**: more ability to run workflows that aren't natively supported by claude code (such as the ralph loop)
