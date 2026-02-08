Issue: Some responses have extra blank lines at the top before the first content. Claude sometimes emits an initial text block containing only `\n\n` before a thinking block. The renderer streams these faithfully, creating inconsistent vertical whitespace.
Status: draft

## Approach

The bug is in `src/display/renderer.rs` `stream_text()` (line ~414). When a whitespace-only text delta arrives:

1. `text_streaming` is `false`, so the method trims leading newlines
2. After trimming, the text is empty, so it returns early (correct — nothing rendered)
3. **But** `self.text_streaming = true` was already set on line 418, before the empty check

Later, when the next block starts (e.g. thinking), `finish_current_block()` sees `text_streaming == true` and emits `\r\n\r\n` — creating blank lines for a block that rendered nothing.

**Fix:** Move `self.text_streaming = true` to after the empty check. Only mark text as streaming if we actually rendered something:

```rust
fn stream_text(&mut self, text: &str) {
    let text = if self.text_streaming {
        Cow::Borrowed(text)
    } else {
        let trimmed = text.trim_start_matches('\n');
        if trimmed.is_empty() {
            return;
        }
        self.text_streaming = true;
        Cow::Borrowed(trimmed)
    };
    let text = text.replace('\n', "\r\n");
    queue!(self.out, Print(&text)).ok();
    self.out.flush().ok();
}
```

After fixing, re-record VCR fixtures and accept updated snapshots (the blank lines at the top of error_handling, mcp_tool, grep_glob should disappear).
