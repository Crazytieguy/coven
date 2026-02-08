Issue: Duplicated input-wait loops: `wait_for_followup` and `wait_for_user_input` in session_loop.rs have nearly identical event loops — extract a shared input-wait helper parameterized by the submit behavior
Status: draft

## Approach

Extract the common event-loop logic from `wait_for_followup` and `wait_for_user_input` into a single generic helper function. The two functions differ only in what happens on `InputAction::Submit` and their return type; everything else (ViewMessage, Cancel, Interrupt/EndSession, Activated/None, terminal error handling) is identical.

### Implementation

1. Define a callback-based helper in `session_loop.rs`:

```rust
/// Outcome from the submit callback, telling the input loop what to do.
enum SubmitOutcome<T> {
    /// Submit was handled; return this value from the loop.
    Done(T),
}

/// Generic input-wait loop. Shows prompt, activates input, and loops on terminal events.
/// The `on_submit` callback is called when the user submits text and decides the return value.
async fn wait_for_input_loop<T, F>(
    input: &mut InputHandler,
    renderer: &mut Renderer,
    term_events: &mut EventStream,
    on_submit: F,
) -> Result<Option<T>>
where
    F: AsyncFn(&str) -> Result<SubmitOutcome<T>>,
```

Wait — async closures are unstable. Instead, use an enum to describe what the caller wants:

2. **Simpler approach — return the text, let callers handle submit:**

Actually the simplest approach: extract the loop that handles ViewMessage/Cancel/Interrupt/EndSession/None, and on Submit just return the text. Both callers already know what to do with the text.

```rust
/// Wait for user to type and submit text, or exit. Returns the submitted text,
/// or None if the user interrupted/ended.
async fn wait_for_text_input(
    input: &mut InputHandler,
    renderer: &mut Renderer,
    term_events: &mut EventStream,
) -> Result<Option<String>> {
    renderer.show_prompt();
    input.activate();

    loop {
        match term_events.next().await {
            Some(Ok(Event::Key(key_event))) => {
                let action = input.handle_key(&key_event);
                match action {
                    InputAction::Submit(text, _) => {
                        renderer.render_user_message(&text);
                        return Ok(Some(text));
                    }
                    InputAction::ViewMessage(n) => {
                        view_message(renderer, n);
                        renderer.show_prompt();
                        input.activate();
                    }
                    InputAction::Cancel => {
                        renderer.show_prompt();
                        input.activate();
                    }
                    InputAction::Interrupt | InputAction::EndSession => {
                        return Ok(None);
                    }
                    InputAction::Activated(_) | InputAction::None => {}
                }
            }
            Some(Ok(_)) => {}
            Some(Err(_)) | None => return Ok(None),
        }
    }
}
```

3. Then rewrite both callers:

```rust
pub async fn wait_for_followup(...) -> Result<FollowUpAction> {
    match wait_for_text_input(input, renderer, term_events).await? {
        Some(text) => {
            runner.send_message(&text).await?;
            state.status = SessionStatus::Running;
            Ok(FollowUpAction::Sent)
        }
        None => Ok(FollowUpAction::Exit),
    }
}

pub async fn wait_for_user_input(...) -> Result<Option<String>> {
    wait_for_text_input(input, renderer, term_events).await
}
```

`wait_for_user_input` becomes a direct passthrough and could be inlined at call sites, but keeping it as a named function is fine for API clarity.

## Questions

### Should `wait_for_user_input` be removed entirely?

After this refactoring, `wait_for_user_input` becomes a trivial wrapper around `wait_for_text_input`. We could either keep it for API clarity or replace its call sites with `wait_for_text_input` directly.

Answer:

## Review

