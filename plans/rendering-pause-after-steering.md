Issue: [P0] I think sometimes rendering pauses after a steering message and there's no way to resume it
Status: draft

## Investigation

The original hypothesis was that this was caused by `flush_event_buffer` not propagating session outcomes — a buffered Result during typing would silently complete, and the subsequent steering message would be sent to a process that produced no more output, causing the event loop to hang.

The `flush_event_buffer` fix has since been implemented. Tracing the scenario now:

1. Claude is running, user starts typing (input activates, events get buffered)
2. Claude sends a Result event while the user is typing (gets buffered)
3. User presses Enter to send steering message
4. `flush_event_buffer()` runs — renders the buffered Result and returns `FlushResult::Completed`
5. `handle_flush_result` receives the Completed result — **intentionally** returns `Ok(None)` (per comment: "if the session completed during the flush, state is WaitingForInput and the match below will send the user's text as a follow-up")
6. The steering message is sent via `runner.send_message()` to the still-alive Claude process
7. Claude processes it as a new turn and produces more output — no hang

Key insight: a Result is a logical completion, not a process exit. The Claude process is still alive and accepts the steering message as another turn. The flush renders the buffered content so the user sees it, and execution continues normally.

The ProcessExit case is also handled: `FlushResult::ProcessExited` causes `handle_flush_result` to return `LoopAction::Return(SessionOutcome::ProcessExited)`, exiting the loop cleanly before attempting to send the steering message.

## Conclusion

This issue appears to be **resolved** by the flush_event_buffer fix. Recommend closing unless the symptom has been observed recently with the current code.

## Review

