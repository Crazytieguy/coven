Issue: [P2] We should unwrap <tool_use_error> when rendering
Status: draft

## Approach

The Claude API wraps tool errors in `<tool_use_error>...</tool_use_error>` XML tags in the `content` field of `tool_result` blocks (visible in `error_handling.vcr` line 62). The `tool_use_result` top-level field is already clean, so the main rendering path (`render_tool_result`) currently avoids showing tags — but only by accident, because it prefers `tool_use_result` over `message.content`.

Two code paths are affected:

1. **`render_subagent_tool_result`** (renderer.rs ~line 307): calls `extract_result_text(item)` directly on content blocks, which contain the XML-tagged `content` field. Tags will show through here.
2. **`render_tool_result` fallback** (renderer.rs ~line 264-268): if `tool_use_result` is empty/missing, it falls back to `msg_content_block`, which also has the tags.

**Fix:** Add a small post-processing step in `extract_result_text()` that strips `<tool_use_error>...</tool_use_error>` wrapping when present. Specifically, if the extracted string starts with `<tool_use_error>` and ends with `</tool_use_error>`, return just the inner text.

This is a single function change (~5 lines) plus a unit test.

No VCR re-recording needed — the existing `error_handling` test case already contains a `<tool_use_error>` tag in its content blocks, but the current test passes because the main path doesn't read from that field. A new unit test for `extract_result_text` directly will cover the stripping logic.

## Questions

None — straightforward.

## Review

