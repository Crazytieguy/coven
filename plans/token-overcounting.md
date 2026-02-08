Issue: I think token count is over-counting, please investigate
Status: draft

## Investigation

Analyzed VCR data and the token computation in `src/lib.rs:53-57`:

```rust
u.input_tokens + u.output_tokens + u.cache_read_input_tokens + u.cache_creation_input_tokens
```

The arithmetic is technically correct — these four fields are disjoint — but the resulting number is misleading:

1. **Cache reads dominate**: A simple "what is 2+2" shows **20k tokens** because ~16k are cached system prompt reads. The big number implies expensive usage when cache reads are nearly free.

2. **Multi-turn inconsistency**: `total_cost_usd` is cumulative across follow-ups but `usage` is per-API-call, making the pairing confusing.

3. **No reliable context usage metric**: We can't derive true context window usage from the available fields — cache reads of the same tokens can happen multiple times across turns, so summing them doesn't reflect actual context consumption.

## Approach

Remove the token count from the result line entirely. Cost already serves as a good proxy for usage, and showing a misleading number is worse than showing nothing.

### Display format change

From:
```
Done  $0.04 · 1.7s · 1 turn · 20k tokens
```
To:
```
Done  $0.04 · 1.7s · 1 turn
```

### Changes

1. **`src/lib.rs`**: Remove token count computation from `process_result_event`. Remove `total_tokens` from `SessionResult` (or whatever struct carries this to the renderer). Remove `format_token_count` helper if it exists.

2. **`src/display/renderer.rs`**: Remove the `· Xk tokens` segment from `render_result`.

3. **`src/protocol/types.rs`**: Remove `usage`-related fields from `SessionResult` if they're only used for the token display.

4. Re-record VCR fixtures and accept snapshot diffs (the token segment disappearing from result lines).

## Questions

None — straightforward removal.

## Review

