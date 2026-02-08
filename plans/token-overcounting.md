Issue: I think token count is over-counting, please investigate
Status: draft

## Investigation

Analyzed VCR data and the token computation in `src/lib.rs:53-57`:

```rust
u.input_tokens + u.output_tokens + u.cache_read_input_tokens + u.cache_creation_input_tokens
```

Where `usage` fields from Claude Code's result event mean:
- `input_tokens`: uncached input tokens (typically very small, e.g. 3-4)
- `cache_read_input_tokens`: tokens read from prompt cache (typically 16-37k — the system prompt)
- `cache_creation_input_tokens`: tokens written to cache (typically 174-4.5k)
- `output_tokens`: model output tokens

The arithmetic is correct — these four fields are disjoint and sum to total tokens processed.

### Finding: Token count is misleading, not wrong

The formula is technically accurate, but the displayed number is misleading:

1. **Cache reads dominate**: A simple "what is 2+2" shows **20k tokens** because ~16k are cached system prompt reads. These cost ~$0.30/MTok (vs $15/MTok for uncached input). The big number implies expensive usage when it's actually cheap.

2. **Multi-turn inconsistency**: In follow-up sessions, `total_cost_usd` is cumulative across all follow-ups but `usage` is per-API-call. The second turn of `multi_turn.vcr` shows `$0.06 · 21k tokens` — cost is for both turns ($0.06) but tokens are only for the second turn (21k, not the cumulative ~42k).

3. **Sub-model tokens missing**: When Haiku is used for tool evaluation, its tokens (~476 in `tool_use.vcr`) aren't included in the `usage` field — only in `modelUsage`.

## Approach

Replace the raw token count with context window usage percentage, showing how close the session was to requiring a compaction. This directly answers "how much context runway did I have?" rather than showing a misleading aggregate number.

### Data source

The `modelUsage` field in result events contains per-model data including `contextWindow` (e.g., 200,000). Example from VCR data:

```json
"modelUsage": {
  "claude-opus-4-6": {
    "inputTokens": 4,
    "outputTokens": 146,
    "cacheReadInputTokens": 37146,
    "cacheCreationInputTokens": 4590,
    "contextWindow": 200000,
    ...
  }
}
```

Total context consumed = `inputTokens + cacheReadInputTokens + cacheCreationInputTokens + outputTokens` summed across all models sharing the primary context window. Context usage = total consumed / contextWindow.

### Display format change

From:
```
Done  $0.04 · 1.7s · 1 turn · 20k tokens
```
To:
```
Done  $0.04 · 1.7s · 1 turn · 21% context
```

If `modelUsage` is unavailable (older Claude Code), drop the token display entirely — cost is already a good proxy.

### Changes

1. **`src/protocol/types.rs`**: Add `ModelUsageEntry` struct with `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`, `context_window` fields (deserializing from camelCase). Add `model_usage: Option<HashMap<String, ModelUsageEntry>>` to `SessionResult`.

2. **`src/lib.rs`**: Compute context usage percentage from `modelUsage`. Sum all token fields across all models for total consumed; use the primary model's `context_window` as the denominator. Pass `Option<u8>` (percentage) to the renderer instead of raw token count.

3. **`src/display/renderer.rs`**: Update `render_result` to display `X% context` when available, or omit the token segment entirely when not.

4. Re-record VCR fixtures and update snapshots.

## Questions

### How should we identify the "primary" model for context window?

`modelUsage` can contain multiple models (e.g., Opus + Haiku). The context window is a property of each model. Options:
- Use the model with the highest total token usage (most likely the primary)
- Use the model with the largest context window
- Use the first model listed

Recommendation: Use the model with the highest total token usage, since that's the model whose context window matters for compaction.

Answer:

## Review

