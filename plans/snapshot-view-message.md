Issue: Add snapshot testing for the :N output
Status: draft

## Approach

The `:N` command formats a message's content and result, then pipes it to an external pager. The formatting logic lives in `view_message()` in `session_loop.rs`. Currently untested.

### What to test

The valuable thing to snapshot is the **formatted content** that gets sent to the pager — not the pager interaction itself. This means testing:
- Correct message lookup (1-based indexing)
- "No message N" for out-of-bounds
- Format: `[N] Label\n\nContent\n\n--- Result ---\n\nResult` for tool calls with results
- Format: `[N] Label\n\nContent` for tool calls without results
- Thinking block content

### Implementation

1. **Extract formatting from pager dispatch**: Refactor `view_message()` to separate the content-building logic into a pure function like `format_message(messages: &[StoredMessage], n: usize) -> String` that returns the formatted text (or an error string for out-of-bounds). The existing `view_message()` calls this, then pipes to the pager.

2. **Add `:N` commands to test cases**: Extend the test message trigger system to support a `view` action. When replaying a VCR test, instead of sending a follow-up message, it would call `format_message()` and append the output to the captured display. Could use a new trigger like `"view:N"` in the `.toml` messages list, or add a separate `views` field.

3. **Alternative — simpler approach**: Rather than integrating into the VCR harness, add unit tests directly for `format_message()` by constructing `StoredMessage` vectors manually and snapshotting the output. This is simpler and doesn't require VCR changes, but doesn't test the integration of message accumulation during replay.

### Recommended path

Go with both:
- Unit tests for `format_message()` with hand-crafted messages (quick, covers formatting logic)
- One integration test using an existing VCR fixture (e.g., `simple_qa`) where we replay the session, then call `format_message()` on the accumulated messages to verify correct accumulation + formatting end-to-end

## Questions

### Should view output be part of VCR snapshots or separate snapshot files?

Embedding `:N` output in the main VCR snapshot (after a `--- :N ---` separator) keeps everything in one place but mixes display output with inspection output. Separate `.view.snap` files are cleaner but add file proliferation.

Answer:

### Should we test the "No message N" error path?

It's trivial logic, but snapshot tests are cheap. Worth a quick unit test?

Answer:

## Review

