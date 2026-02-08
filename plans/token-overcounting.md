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

Replace the single "total tokens" number with a more useful breakdown showing input/output split, since that's what users care about for understanding cost and usage.

Display format change from:
```
Done  $0.04 · 1.7s · 1 turn · 20k tokens
```
To:
```
Done  $0.04 · 1.7s · 1 turn · 20k in / 0k out
```

This makes the cache-dominated input visible vs actual model output. We'd use `modelUsage` (cumulative, all models) when available, falling back to `usage` fields.

### Changes

1. **`src/protocol/types.rs`**: Add `ModelUsageEntry` struct and `model_usage` field to `SessionResult` to parse the `modelUsage` map from result events.

2. **`src/lib.rs`**: Compute cumulative input and output tokens from `modelUsage` (summing across all models), falling back to `usage` fields.

3. **`src/display/renderer.rs`**: Update `render_result` to accept separate input/output token counts and format as `Xk in / Yk out`.

4. Re-record VCR fixtures and update snapshots.

## Questions

### Should we show cache breakdown or just input/output?

We could show a three-way split like `16k cached / 4k in / 0k out`, but that adds complexity. The simpler `20k in / 0k out` still conveys the key info (output is small relative to input). A middle ground is showing just `in / out` since cost already captures the financial impact.

Recommendation: `in / out` only, since caching is an implementation detail users shouldn't need to think about.

Answer:

### Should we use modelUsage for cumulative accuracy?

`modelUsage` in the result event is cumulative across all turns and includes all sub-models (Haiku, etc.). Using it would fix the multi-turn inconsistency (tokens would be cumulative like cost). The downside is it's a different field with different structure (`camelCase` keys, nested per-model map).

Recommendation: Yes, use `modelUsage` for accuracy, fall back to `usage` for older Claude Code versions.

Answer:

## Review

