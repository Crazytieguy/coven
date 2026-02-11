- [P2] Durable conflict logging: conflict files and resolution outcomes are only rendered to the terminal. Consider logging them to a file for post-mortem analysis. (plan: plans/durable-conflict-logging.md)
- [P2] Create a features / intended behavior reference document so it's clear what each command mode supports (interaction modes, session lifecycle, rendering expectations). Makes it easier to verify whether code reflects intended behavior. (plan: plans/features-reference-doc.md)
- [P2] SessionConfig construction repeated across run.rs, ralph.rs, and worker.rs with similar field patterns. Consider a builder or factory method to reduce duplication.
- [P1] The test cases folder is currently a flat list of files, and it's hard for me to navigate. I want to organize all tests into a neat folder structure (plan: plans/test-folder-structure.md)
- [P1] Regular terminal keyboard nativagion doesn't work when giving interactive input. It would be nice to be able to use the regular terminal keybindings for things like jumping back a word or deleting a word etc (plan: plans/readline-keybindings.md)
- [P1] :N view for common claude code tools should display in a nicer format than just the raw json (plan: plans/pretty-tool-view.md)
- [P2] Interrupt -> follow up message doesn't need to re-pring the session id (it's the same id). Simple change, no plan needed
- [P1] When sending a user message that wraps on multiple lines, only the last line is cleared before the message is repeated. So it looks something like this: """
[182] ▶ TaskOutput  b76cc75

[interrupted]
> I think it's taking way too much time. I've killed the task. See if you can identify an issue that would cause an infinite loop or would c> I think it's taking way too much time. I've killed the task. See if you can identify an issue that would cause an infinite loop or would cause the model to run for a very long time
Session 39641608-a4f1-40a4-a775-1c3ab94f555c (claude-opus-4-6)
""" (plan: plans/multiline-input-clearing.md)