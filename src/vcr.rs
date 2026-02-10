use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::Path;

use anyhow::{Result, bail};
use crossterm::event::Event;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

use crate::event::AppEvent;
use crate::session::runner::SessionRunner;

/// Default model used for VCR test recordings. Shared between record-vcr and the test harness
/// so that spawn args match during replay assertion.
pub const DEFAULT_TEST_MODEL: &str = "claude-haiku-4-5-20251001";

// ── Recordable trait ────────────────────────────────────────────────────

/// Allows both serializable types and non-serializable types (like process
/// handles) to work with `vcr.call()`.
pub trait Recordable: Sized {
    type Recorded: Serialize + DeserializeOwned;
    fn to_recorded(&self) -> Result<Self::Recorded>;
    fn from_recorded(recorded: Self::Recorded) -> Result<Self>;
}

/// Blanket implementation for any type implementing `Serialize + DeserializeOwned`.
/// Uses `serde_json::Value` as the intermediate representation, avoiding a `Clone`
/// requirement.
impl<T: Serialize + DeserializeOwned> Recordable for T {
    type Recorded = Value;

    fn to_recorded(&self) -> Result<Value> {
        Ok(serde_json::to_value(self)?)
    }

    fn from_recorded(v: Value) -> Result<Self> {
        Ok(serde_json::from_value(v)?)
    }
}

// ── RecordableError trait ─────────────────────────────────────────────

/// Allows error types to be fully serialized/deserialized through VCR recording,
/// preserving variant structure for pattern matching. Use with `vcr.call_typed_err()`
/// for operations where callers need to match on specific error variants.
pub trait RecordableError: std::error::Error + Sized {
    type Recorded: Serialize + DeserializeOwned;
    fn to_recorded_err(&self) -> Result<Self::Recorded>;
    fn from_recorded_err(recorded: Self::Recorded) -> Result<Self>;
}

/// Blanket implementation for any error type implementing `Serialize + DeserializeOwned`.
impl<E: Serialize + DeserializeOwned + std::error::Error> RecordableError for E {
    type Recorded = Value;

    fn to_recorded_err(&self) -> Result<Value> {
        Ok(serde_json::to_value(self)?)
    }

    fn from_recorded_err(v: Value) -> Result<Self> {
        Ok(serde_json::from_value(v)?)
    }
}

// ── Manual Recordable impls ─────────────────────────────────────────────

/// `SessionRunner` records as `()` — in replay mode, a stub with no child/stdin
/// is returned. All actual operations on the stub are either no-ops (`close_input`,
/// `wait`, `kill`) or bypassed by VCR (`send_message` is wrapped in `vcr.call()`).
impl Recordable for SessionRunner {
    type Recorded = ();

    fn to_recorded(&self) -> Result<()> {
        Ok(())
    }

    fn from_recorded((): ()) -> Result<Self> {
        Ok(SessionRunner::stub())
    }
}

// ── VCR entry (one line in the NDJSON file) ─────────────────────────────

#[derive(Serialize, Deserialize)]
struct VcrEntry {
    label: String,
    args: Value,
    result: Value,
}

// ── VcrContext ───────────────────────────────────────────────────────────

/// Operating mode for the VCR context.
enum VcrMode {
    /// Production — just execute operations.
    Live,
    /// Execute and record args + results.
    Record(RefCell<Vec<VcrEntry>>),
    /// Return recorded values, assert arguments match.
    Replay(RefCell<ReplayState>),
}

struct ReplayState {
    entries: Vec<VcrEntry>,
    position: usize,
}

/// A VCR context threaded through command functions. Records or replays
/// all external I/O operations via the `call()` method.
pub struct VcrContext {
    mode: VcrMode,
    trigger_controller: Option<RefCell<TriggerController>>,
}

impl VcrContext {
    /// Create a live context (production mode — operations execute normally).
    pub fn live() -> Self {
        Self {
            mode: VcrMode::Live,
            trigger_controller: None,
        }
    }

    /// Create a recording context that captures all operations.
    pub fn record() -> Self {
        Self {
            mode: VcrMode::Record(RefCell::new(Vec::new())),
            trigger_controller: None,
        }
    }

    /// Create a recording context with a trigger controller for scripted input.
    pub fn record_with_triggers(controller: TriggerController) -> Self {
        Self {
            mode: VcrMode::Record(RefCell::new(Vec::new())),
            trigger_controller: Some(RefCell::new(controller)),
        }
    }

    /// Create a replay context from recorded NDJSON data.
    pub fn replay(data: &str) -> Result<Self> {
        let mut entries = Vec::new();
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            entries.push(serde_json::from_str(line)?);
        }
        Ok(Self {
            mode: VcrMode::Replay(RefCell::new(ReplayState {
                entries,
                position: 0,
            })),
            trigger_controller: None,
        })
    }

    /// Write the recording to an NDJSON file.
    pub fn write_recording(&self, path: &Path) -> Result<()> {
        let VcrMode::Record(ref entries) = self.mode else {
            bail!("write_recording called on non-Record VcrContext");
        };
        let entries = entries.borrow();
        let mut output = String::new();
        for entry in entries.iter() {
            output.push_str(&serde_json::to_string(entry)?);
            output.push('\n');
        }
        std::fs::write(path, output)?;
        Ok(())
    }

    /// The core VCR method. All external operations go through this.
    ///
    /// - **Live**: calls `f` and returns the result.
    /// - **Record**: calls `f`, records args and result, returns the result.
    /// - **Replay**: asserts args match the recording, returns the recorded result.
    ///
    /// Errors are always recorded as their display string and replayed via `anyhow!()`.
    pub async fn call<A, T>(
        &self,
        label: &str,
        args: A,
        f: impl AsyncFnOnce(&A) -> Result<T>,
    ) -> Result<T>
    where
        A: Recordable,
        A::Recorded: PartialEq + Debug,
        T: Recordable,
    {
        match &self.mode {
            VcrMode::Live => f(&args).await,
            VcrMode::Record(entries) => {
                let result = f(&args).await;
                let recorded_result: std::result::Result<T::Recorded, String> = match &result {
                    Ok(t) => Ok(t.to_recorded()?),
                    Err(e) => Err(format!("{e:#}")),
                };
                self.push_entry(entries, label, &args, &recorded_result)?;
                result
            }
            VcrMode::Replay(state) => {
                let entry_result = Self::advance_replay(state, label, &args)?;
                let recorded_result: std::result::Result<T::Recorded, String> =
                    serde_json::from_value(entry_result)?;
                match recorded_result {
                    Ok(t) => Ok(T::from_recorded(t)?),
                    Err(msg) => Err(anyhow::anyhow!("{msg}")),
                }
            }
        }
    }

    /// Like [`call()`](Self::call), but preserves typed errors through recording/replay.
    ///
    /// Use this for operations where callers need to match on specific error
    /// variants (e.g., `WorktreeError::RebaseConflict`). Errors are serialized
    /// using their [`RecordableError`] impl instead of being stringified.
    ///
    /// Returns `Result<std::result::Result<T, E>>` — the outer `Result` carries
    /// VCR infrastructure errors (label/args mismatch, exhausted recording),
    /// the inner `Result` carries the typed application error.
    pub async fn call_typed_err<A, T, E>(
        &self,
        label: &str,
        args: A,
        f: impl AsyncFnOnce(&A) -> std::result::Result<T, E>,
    ) -> Result<std::result::Result<T, E>>
    where
        A: Recordable,
        A::Recorded: PartialEq + Debug,
        T: Recordable,
        E: RecordableError,
    {
        match &self.mode {
            VcrMode::Live => Ok(f(&args).await),
            VcrMode::Record(entries) => {
                let result = f(&args).await;
                let recorded_result: std::result::Result<T::Recorded, E::Recorded> = match &result {
                    Ok(t) => Ok(t.to_recorded()?),
                    Err(e) => Err(e.to_recorded_err()?),
                };
                self.push_entry(entries, label, &args, &recorded_result)?;
                Ok(result)
            }
            VcrMode::Replay(state) => {
                let entry_result = Self::advance_replay(state, label, &args)?;
                let recorded_result: std::result::Result<T::Recorded, E::Recorded> =
                    serde_json::from_value(entry_result)?;
                match recorded_result {
                    Ok(t) => Ok(Ok(T::from_recorded(t)?)),
                    Err(e) => Ok(Err(E::from_recorded_err(e)?)),
                }
            }
        }
    }

    /// Advance the replay position and validate that the label and args match.
    /// Returns the raw recorded result `Value` for the caller to deserialize.
    fn advance_replay<A>(state: &RefCell<ReplayState>, label: &str, args: &A) -> Result<Value>
    where
        A: Recordable,
        A::Recorded: PartialEq + Debug,
    {
        let (entry_label, entry_args, entry_result, pos) = {
            let mut state = state.borrow_mut();
            anyhow::ensure!(
                state.position < state.entries.len(),
                "VCR replay exhausted: expected more entries after position {}",
                state.position
            );
            let pos = state.position;
            let entry = &state.entries[pos];
            let result = (
                entry.label.clone(),
                entry.args.clone(),
                entry.result.clone(),
                pos,
            );
            state.position += 1;
            result
        };

        anyhow::ensure!(
            entry_label == label,
            "VCR label mismatch at position {pos}: expected '{entry_label}', got '{label}'"
        );

        let recorded_args: A::Recorded = serde_json::from_value(entry_args)?;
        let actual_args = args.to_recorded()?;
        anyhow::ensure!(
            recorded_args == actual_args,
            "VCR args mismatch for '{label}' at position {pos}: expected {recorded_args:?}, got {actual_args:?}"
        );

        Ok(entry_result)
    }

    /// Record a VCR entry and notify the trigger controller.
    fn push_entry<A, R: Serialize>(
        &self,
        entries: &RefCell<Vec<VcrEntry>>,
        label: &str,
        args: &A,
        recorded_result: &R,
    ) -> Result<()>
    where
        A: Recordable,
    {
        let entry = VcrEntry {
            label: label.to_string(),
            args: serde_json::to_value(args.to_recorded()?)?,
            result: serde_json::to_value(recorded_result)?,
        };
        let result_value = entry.result.clone();
        entries.borrow_mut().push(entry);
        if let Some(ref tc) = self.trigger_controller {
            tc.borrow_mut().check(label, &result_value);
        }
        Ok(())
    }

    /// Whether this context is in live (production) mode.
    pub fn is_live(&self) -> bool {
        matches!(&self.mode, VcrMode::Live)
    }

    /// Whether this context is in replay mode.
    pub fn is_replay(&self) -> bool {
        matches!(&self.mode, VcrMode::Replay(_))
    }

    /// Whether this context is in record mode.
    pub fn is_record(&self) -> bool {
        matches!(&self.mode, VcrMode::Record(_))
    }
}

// ── IoEvent ─────────────────────────────────────────────────────────────

/// Unified event from either the Claude process or the terminal.
/// Replaces the `tokio::select!` between claude events and terminal events
/// with a single VCR-able type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IoEvent {
    /// An event from the claude process (parsed NDJSON).
    Claude(AppEvent),
    /// A terminal key/resize/etc event.
    Terminal(Event),
}

// ── Io struct ───────────────────────────────────────────────────────────

/// Owns the event channels and provides a unified `next_event()` method.
/// In production, terminal events come from a crossterm adapter task.
/// During recording, the `TriggerController` pushes scripted events.
/// During replay, `next_event()` is never called (VCR returns recorded events).
pub struct Io {
    event_rx: mpsc::UnboundedReceiver<AppEvent>,
    term_rx: mpsc::UnboundedReceiver<Event>,
    /// Kept alive so `event_rx.recv()` doesn't return `None` while idle.
    idle_tx: Option<mpsc::UnboundedSender<AppEvent>>,
}

impl Io {
    pub fn new(
        event_rx: mpsc::UnboundedReceiver<AppEvent>,
        term_rx: mpsc::UnboundedReceiver<Event>,
    ) -> Self {
        Self {
            event_rx,
            term_rx,
            idle_tx: None,
        }
    }

    /// Create a dummy Io for replay mode (channels are immediately closed).
    pub fn dummy() -> Self {
        let (_tx1, rx1) = mpsc::unbounded_channel();
        let (_tx2, rx2) = mpsc::unbounded_channel();
        Self {
            event_rx: rx1,
            term_rx: rx2,
            idle_tx: None,
        }
    }

    /// Get the next event from either the Claude process or the terminal.
    pub async fn next_event(&mut self) -> Result<IoEvent> {
        tokio::select! {
            event = self.event_rx.recv() => {
                Ok(IoEvent::Claude(
                    event.unwrap_or(AppEvent::ProcessExit(None))
                ))
            }
            event = self.term_rx.recv() => {
                match event {
                    Some(e) => Ok(IoEvent::Terminal(e)),
                    None => Ok(IoEvent::Claude(AppEvent::ProcessExit(None))),
                }
            }
        }
    }

    /// Replace the event channel and return the new sender.
    ///
    /// The old receiver (and any stale events like `ProcessExit`) is dropped.
    pub fn replace_event_channel(&mut self) -> mpsc::UnboundedSender<AppEvent> {
        self.idle_tx = None;
        let (tx, rx) = mpsc::unbounded_channel();
        self.event_rx = rx;
        tx
    }

    /// Discard the current event channel (draining any stale events) and
    /// replace it with an idle channel that blocks on `recv()` without
    /// returning `None`.
    ///
    /// Call this after killing a runner and before `wait_for_user_input`
    /// to prevent a stale `ProcessExit` from immediately ending the wait.
    pub fn clear_event_channel(&mut self) {
        let (tx, rx) = mpsc::unbounded_channel();
        self.event_rx = rx;
        self.idle_tx = Some(tx);
    }
}

// ── TriggerController ───────────────────────────────────────────────────

/// Injects scripted terminal input during recording based on trigger conditions.
/// Watches recorded events and pushes key events into the terminal channel
/// when triggers match.
pub struct TriggerController {
    triggers: Vec<PendingTrigger>,
    term_tx: mpsc::UnboundedSender<Event>,
    /// When true, automatically inject Ctrl+D after all triggers have fired
    /// and a result event is seen. Used for `run` mode recordings.
    auto_exit: bool,
}

struct PendingTrigger {
    /// JSON subset pattern to match against the VCR call result.
    condition: Option<Value>,
    /// If set, the trigger only fires when the VCR call has this label.
    label: Option<String>,
    text: String,
    mode: TriggerInputMode,
    fired: bool,
}

/// Whether a triggered message is a steering (Enter), follow-up (Alt+Enter),
/// exit (Ctrl+D), or interrupt (Ctrl+C followed by resume text).
#[derive(Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TriggerInputMode {
    #[default]
    Followup,
    Steering,
    Exit,
    Interrupt,
}

impl TriggerController {
    /// Create a new trigger controller from test case messages.
    pub fn new(messages: &[TestMessage], term_tx: mpsc::UnboundedSender<Event>) -> Result<Self> {
        let triggers = messages
            .iter()
            .map(|m| {
                anyhow::ensure!(
                    m.trigger.is_some() || m.label.is_some(),
                    "trigger message must have at least one of `trigger` or `label`"
                );
                let condition = m
                    .trigger
                    .as_ref()
                    .map(|t| serde_json::from_str(t))
                    .transpose()?;
                Ok(PendingTrigger {
                    condition,
                    label: m.label.clone(),
                    text: m.content.clone(),
                    mode: m.mode,
                    fired: false,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            triggers,
            term_tx,
            auto_exit: false,
        })
    }

    /// Enable auto-exit: inject Ctrl+D after all triggers fired and result seen.
    #[must_use]
    pub fn with_auto_exit(mut self) -> Self {
        self.auto_exit = true;
        self
    }

    /// Check a recorded VCR call against triggers and inject terminal events.
    pub fn check(&mut self, vcr_label: &str, recorded_result: &Value) {
        // Collect triggers to fire first to avoid borrow conflict
        let to_inject: Vec<(String, TriggerInputMode)> = self
            .triggers
            .iter_mut()
            .filter(|t| {
                if t.fired {
                    return false;
                }
                if t.label.as_deref().is_some_and(|l| l != vcr_label) {
                    return false;
                }
                match &t.condition {
                    Some(cond) => is_subset(cond, recorded_result),
                    None => true, // label-only trigger, already matched above
                }
            })
            .map(|t| {
                t.fired = true;
                (t.text.clone(), t.mode)
            })
            .collect();

        let any_fired_this_call = !to_inject.is_empty();
        for (text, mode) in &to_inject {
            match mode {
                TriggerInputMode::Exit => inject_exit(&self.term_tx),
                TriggerInputMode::Interrupt => {
                    inject_interrupt(&self.term_tx);
                    if !text.is_empty() {
                        inject_text(&self.term_tx, text, TriggerInputMode::Steering);
                    }
                }
                _ => inject_text(&self.term_tx, text, *mode),
            }
        }

        // Auto-exit: if all triggers have fired, none fired THIS call, and this
        // looks like a result event, inject Ctrl+D to signal exit.
        if self.auto_exit && !any_fired_this_call && self.triggers.iter().all(|t| t.fired) {
            let result_pattern =
                serde_json::json!({"Ok": {"Claude": {"Claude": {"type": "result"}}}});
            if is_subset(&result_pattern, recorded_result) {
                inject_exit(&self.term_tx);
            }
        }
    }
}

/// Inject text as individual key events followed by Enter.
fn inject_text(term_tx: &mpsc::UnboundedSender<Event>, text: &str, mode: TriggerInputMode) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    for ch in text.chars() {
        let event = Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        let _ = term_tx.send(event);
    }

    let enter_modifiers = match mode {
        TriggerInputMode::Followup => KeyModifiers::ALT,
        TriggerInputMode::Steering => KeyModifiers::NONE,
        TriggerInputMode::Exit | TriggerInputMode::Interrupt => {
            unreachable!("Exit/Interrupt triggers are handled in check(), not inject_text()")
        }
    };
    let enter = Event::Key(KeyEvent {
        code: KeyCode::Enter,
        modifiers: enter_modifiers,
        kind: KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    });
    let _ = term_tx.send(enter);
}

/// Inject Ctrl+D (`EndSession` signal) into the terminal channel.
fn inject_exit(term_tx: &mpsc::UnboundedSender<Event>) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    let exit = Event::Key(KeyEvent {
        code: KeyCode::Char('d'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    });
    let _ = term_tx.send(exit);
}

/// Inject Ctrl+C (`Interrupt` signal) into the terminal channel.
fn inject_interrupt(term_tx: &mpsc::UnboundedSender<Event>) {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    let ctrl_c = Event::Key(KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::CONTROL,
        kind: KeyEventKind::Press,
        state: crossterm::event::KeyEventState::NONE,
    });
    let _ = term_tx.send(ctrl_c);
}

/// Recursive subset matching: returns true if `pattern` is a subset of `event`.
fn is_subset(pattern: &Value, event: &Value) -> bool {
    match (pattern, event) {
        (Value::Object(p), Value::Object(e)) => p
            .iter()
            .all(|(k, v)| e.get(k).is_some_and(|ev| is_subset(v, ev))),
        _ => pattern == event,
    }
}

// ── Test case types ──────────────────────────────────────────────────────

/// Test case definition loaded from a `.toml` file.
#[derive(Deserialize, Default)]
pub struct TestCase {
    /// Configuration for a standard run.
    pub run: Option<RunConfig>,
    /// Configuration for ralph loop mode.
    pub ralph: Option<RalphConfig>,
    /// Configuration for worker mode.
    pub worker: Option<WorkerTestConfig>,
    /// Display/renderer configuration for test replay.
    #[serde(default)]
    pub display: DisplayConfig,
    /// Files to create in the working directory before recording.
    #[serde(default)]
    pub files: HashMap<String, String>,
    /// Additional messages to send during the session (follow-ups, steering).
    #[serde(default)]
    pub messages: Vec<TestMessage>,
    /// Message labels to snapshot via `:N` or `:P/C` view commands.
    #[serde(default)]
    pub views: Vec<String>,
}

/// Display configuration for test replay (not used during recording).
#[derive(Deserialize, Default)]
pub struct DisplayConfig {
    /// Whether to stream thinking text inline.
    #[serde(default)]
    pub show_thinking: bool,
}

/// CLI configuration for a standard run (mirrors coven's CLI args).
#[derive(Deserialize)]
pub struct RunConfig {
    /// Prompt to send to claude.
    pub prompt: String,
    /// Extra arguments to pass through to claude.
    #[serde(default)]
    pub claude_args: Vec<String>,
}

/// CLI configuration for ralph loop mode (mirrors coven's ralph subcommand args).
#[derive(Deserialize)]
pub struct RalphConfig {
    /// Prompt to send on each iteration.
    pub prompt: String,
    /// Tag that signals loop completion.
    #[serde(default = "default_break_tag")]
    pub break_tag: String,
    /// Extra arguments to pass through to claude.
    #[serde(default)]
    pub claude_args: Vec<String>,
}

fn default_break_tag() -> String {
    "break".to_string()
}

/// CLI configuration for worker mode (mirrors coven's worker subcommand args).
#[derive(Deserialize)]
pub struct WorkerTestConfig {
    /// Extra arguments to pass through to claude.
    #[serde(default)]
    pub claude_args: Vec<String>,
}

/// A message to send during a recording session.
#[derive(Deserialize)]
pub struct TestMessage {
    /// The message content.
    pub content: String,
    /// JSON subset pattern to match against the VCR call result.
    /// At least one of `trigger` or `label` must be set.
    #[serde(default)]
    pub trigger: Option<String>,
    /// If set, only match VCR calls with this label.
    #[serde(default)]
    pub label: Option<String>,
    /// How to send: "followup" (Alt+Enter) or "steering" (Enter). Defaults to "followup".
    #[serde(default)]
    pub mode: TriggerInputMode,
}

impl TestCase {
    /// Whether this is a ralph test case.
    pub fn is_ralph(&self) -> bool {
        self.ralph.is_some()
    }

    /// Whether this is a worker test case.
    pub fn is_worker(&self) -> bool {
        self.worker.is_some()
    }
}
