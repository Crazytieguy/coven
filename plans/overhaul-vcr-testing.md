Issue: [P0] Generic VCR testing infrastructure: VcrContext with a single `call()` method that records/replays all external I/O (claude sessions, git operations, terminal input). Migrate existing tests to exercise real command code paths. First target: single-worker dispatch → agent → land pipeline.
Status: approved

## Guiding principles

1. **Test actual behavior**: Tests should exercise real command code paths (run, ralph, worker), not just rendering. The VCR system should be a seam at the I/O boundary, not a bypass of application logic.
2. **One generic mechanism**: Avoid separate abstractions for different kinds of I/O (sessions, git, terminal). A single `VcrContext` with a generic `call()` method handles all external operations.
3. **Record everything**: The VCR captures both inputs (what we receive: claude events, git results) and outputs (what we send: prompts, messages, git arguments). During replay, inputs are returned from the recording and outputs are asserted against it.
4. **Fragility is a feature**: If the code changes what operations it performs or what arguments it passes, recorded VCR files break. This is intentional — breakage means the test should be re-recorded.
5. **Reuse over invention**: Use serde for serialization, serde_json::Value for event matching, existing crossterm serde support for terminal events. Minimize custom DSLs and query languages.

## Scope

Start with a single worker running the dispatch → agent → land pipeline. Multi-worker concurrent testing is deferred — it can be added later once the single-worker infrastructure is solid.

Existing VCR tests (simple_qa, ralph_break, multi_turn, steering, etc.) should be migrated to the new infrastructure so they exercise real command code paths.

## Design

### VcrContext and `call()`

A `VcrContext` is threaded as an explicit parameter through all command functions. It operates in three modes: `Live` (production — just execute), `Record` (execute and record), `Replay` (return recorded values, assert arguments match).

All external operations go through a single async method:

```rust
impl VcrContext {
    async fn call<A, T, E>(
        &self,
        label: &str,
        args: A,
        f: impl AsyncFnOnce(&A) -> Result<T, E>,
    ) -> Result<T, E>
    where
        A: Recordable, A::Recorded: PartialEq + Debug,
        T: Recordable,
        E: Recordable,
    {
        match &self.mode {
            VcrMode::Live => f(&args).await,
            VcrMode::Record(rec) => {
                let result = f(&args).await;
                // Record args and result
                result
            }
            VcrMode::Replay(player) => {
                // Assert args match, return recorded result
            }
        }
    }
}
```

Only an async variant is needed — sync operations wrapped in an async block work trivially. The return type is always `Result<T, E>`, which is part of the contract.

Usage examples:

```rust
// Git operation — records path arg and land result:
let result = vcr.call("land", path.to_path_buf(), async |p| worktree::land(p)).await?;

// Claude event — records the event:
let event = vcr.call("next_event", (), async |_| io.next_event_raw().await).await?;

// Message to claude — records the text we sent:
vcr.call("send_message", text, async |t| runner.send_message(t)).await?;
```

No feature gate — `VcrMode::Live` has trivial overhead (one well-predicted branch per call).

### Recordable trait

Allows both serializable types and non-serializable types (like process handles) to work with `vcr.call()`.

```rust
trait Recordable: Sized {
    type Recorded: Serialize + DeserializeOwned;
    fn to_recorded(&self) -> Self::Recorded;
    fn from_recorded(recorded: Self::Recorded) -> Self;
}
```

**Blanket implementation** for any type implementing `Serialize + DeserializeOwned` — uses `serde_json::Value` as the intermediate representation, avoiding a `Clone` requirement:

```rust
impl<T: Serialize + DeserializeOwned> Recordable for T {
    type Recorded = serde_json::Value;
    fn to_recorded(&self) -> Value { serde_json::to_value(self).unwrap() }
    fn from_recorded(v: Value) -> Self { serde_json::from_value(v).unwrap() }
}
```

**Manual implementations** for special types:

- `SessionRunner`: records as `Option<String>` (session ID), replays as a stub with optional child/stdin fields set to `None`.
- `anyhow::Error`: records as display string, replays via `anyhow!()`.
- Lock guards and similar RAII types: record as `()`, replay as no-op wrappers.

### Unified IoEvent model

Replace the current `tokio::select!` between claude events and terminal events with a single VCR-able call:

```rust
enum IoEvent {
    Claude(AppEvent),
    Terminal(crossterm::event::Event),
}
```

An `Io` struct owns the claude event channel and terminal event channel. Its `next_event_raw()` method does the select internally. This call is wrapped in `vcr.call()`, which records the exact interleaving of claude and terminal events. During replay, the recorded interleaving is reproduced exactly.

Terminal events come from an `mpsc::UnboundedReceiver<Event>` in all modes:
- **Production**: An adapter task reads from `crossterm::EventStream` and forwards to the channel.
- **Recording**: A `TriggerController` pushes scripted key events into the channel (see below).
- **Replay**: No channel needed — `next_event_raw()` is never called (VCR returns recorded events directly).

### Terminal input during recording

During recording, scripted terminal input is injected via a `TriggerController`:

1. The VCR recording layer notifies the `TriggerController` after recording each event.
2. The `TriggerController` checks the recorded event against trigger conditions.
3. If a trigger matches, it pushes key events (individual keystrokes) into the terminal event channel.
4. The next `io.next_event_raw()` call picks them up via select.

Trigger conditions use **subset matching on serialized events** — a TOML table is deserialized as `serde_json::Value` and matched against the recorded event using a simple recursive subset check:

```rust
fn is_subset(pattern: &Value, event: &Value) -> bool {
    match (pattern, event) {
        (Value::Object(p), Value::Object(e)) => {
            p.iter().all(|(k, v)| e.get(k).is_some_and(|ev| is_subset(v, ev)))
        }
        _ => pattern == event,
    }
}
```

This reuses serde_json entirely — no custom query language. Test case TOML example:

```toml
[[inputs]]
when = { Ok = { Claude = { Result = {} } } }
text = "What about 3+3?"
```

The exact serialized event structure and whether convenience shorthands are needed are implementation details to be worked out during development.

### Renderer output

`Renderer<W: Write>` is already generic. Tests pass a buffer (e.g., `Vec<u8>`) and snapshot the output using insta. This is the primary assertion on our program's output, separate from the VCR arg assertions.

### VCR file format

NDJSON, one line per VCR entry. Each entry contains:
- `label`: stringified expression identifying the operation
- `args`: serialized arguments (our output — asserted on replay)
- `result`: serialized return value (our input — replayed)

The exact schema is an implementation detail. Existing `.vcr` files are incompatible with this format and will be re-recorded.

### Test case format

Tests are defined in TOML and exercise real coven subcommands:

```toml
[test]
command = "run"        # or "ralph", "worker"
prompt = "What is 2+2?"

# Optional scripted terminal input
[[inputs]]
when = { ... }
text = "follow-up message"

# Optional files to set up in test directory
[files]
"TODO.md" = "# Tasks\n- [ ] Do something\n"
```

The exact structure — especially for worker tests (agent definitions, issue files, git setup) — is an implementation detail to be worked out.

### record-vcr changes

The `record-vcr` binary runs the actual command functions (run, ralph, worker) with `VcrMode::Record`, rather than spawning claude directly. This ensures recordings capture the full behavior including orchestration logic, git operations, and terminal interaction.

### Existing test migration

All existing VCR tests are migrated to the new infrastructure. They exercise real command code paths instead of just replaying events through the renderer. The `.toml` configs are updated to the new format, `.vcr` files are re-recorded, and `.snap` files are regenerated.

## Implementation details to work out

These are not design blockers — they should be resolved during implementation:

- **Async closure syntax**: Verify `AsyncFnOnce` works in edition 2024, or determine the right syntax for async closures in trait bounds.
- **Serialization coverage**: Many types need `Serialize + Deserialize` derives added (protocol event types, `SessionConfig`, `WorktreeError`, `LandResult`, etc.). Enable crossterm's `serde` feature for terminal event types.
- **SessionRunner stub**: Making `child` field optional so `SessionRunner::from_recorded()` can construct a stub. Methods like `close_input()`, `wait()`, `kill()` need to handle the stub case gracefully.
- **VcrContext mutability**: Interior mutability (RefCell/Mutex) vs `&mut` access for recording. Needs to avoid borrow conflicts with closures.
- **Trigger matching ergonomics**: The raw serde serialization of enum variants may be verbose for trigger patterns, but this is acceptable — verbose patterns are explicit and don't require learning special syntax.
- **Test parallelism**: Ensure VCR replay tests can run in parallel (each test has its own VcrContext, no shared state).
- **Worker test setup**: What agent definitions, issue files, and git state are needed for the first worker test case.

## Implementation steps

1. Define `Recordable` trait with blanket impl and manual impls
2. Implement `VcrContext` with `call()` method and three modes (Live, Record, Replay)
3. Add `Serialize + Deserialize` to protocol types, error types, and enable crossterm serde feature
4. Define `IoEvent` enum and `Io` struct, refactor event loop to use unified `next_event()`
5. Refactor terminal input to use channel instead of `EventStream` directly
6. Thread `VcrContext` through command functions (run, ralph, worker)
7. Wrap all external operations with `vcr.call()` (session spawn, next_event, send_message, git operations, etc.)
8. Implement `TriggerController` with subset matching for scripted input during recording
9. Update `record-vcr` to run real command functions with `VcrMode::Record`
10. Update test case TOML format and test harness to use `VcrMode::Replay`
11. Re-record all existing test cases and regenerate snapshots
12. Add first worker test case: single worker, dispatch → agent → land

## Questions (resolved)

### Sequential vs concurrent test execution

Single worker is sufficient for the first pass. Multi-worker concurrent testing is deferred.

### Session injection approach

A generic `VcrContext` with `vcr.call()` replaces the need for a separate `SessionFactory` or `Session` trait. All external operations (sessions, git, terminal) use the same mechanism.

### Scope of first test case

Minimal: single worker, one dispatch + one agent + land. Proves the pipeline works end-to-end through real command code.
