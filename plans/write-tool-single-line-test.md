Issue: Add a VCR test case covering the Write tool with single-line content to verify the "(1 line)" singular display
Status: draft

## Approach

Create a new VCR test case `write_single_line` that exercises the Write tool with single-line content, verifying the renderer shows "(1 line)" rather than "(1 lines)".

### Test case config (`tests/cases/write_single_line.toml`)

```toml
[run]
prompt = "Write a file called hello.txt containing just the text 'Hello, world!'"
```

Simple prompt that should produce a single Write tool call with one line of content. No files or messages needed.

### Steps

1. Create `tests/cases/write_single_line.toml` with the config above.
2. Run `cargo run --bin record-vcr write_single_line` to record the fixture.
3. Add `vcr_test!(write_single_line);` to `tests/vcr_test.rs`.
4. Run `cargo test` to generate the snapshot, review it to confirm it shows "(1 line)".
5. Accept with `cargo insta accept`.
6. Verify `cargo clippy` and `cargo test` pass.

### Expected snapshot content

The snapshot should show a tool line like:
```
[1] ▶ Write  hello.txt (1 line)
```

Confirming the singular "(1 line)" display works correctly.

## Questions

None — this is straightforward.

## Review

