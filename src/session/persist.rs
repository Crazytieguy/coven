use std::io::{BufRead, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use notify::{RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

use crate::vcr::VcrContext;

/// How long to wait for the session file to be updated before giving up.
const PERSIST_TIMEOUT: Duration = Duration::from_secs(5);

/// Compute the path to a Claude Code session JSONL file.
///
/// Claude Code stores sessions at `~/.claude/projects/<encoded-path>/<session-id>.jsonl`
/// where `<encoded-path>` is the canonical working directory with `/` replaced by `-`.
fn session_file_path(working_dir: &Path, session_id: &str) -> Result<PathBuf> {
    let canonical = working_dir
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", working_dir.display()))?;
    let encoded = canonical.display().to_string().replace('/', "-");
    let home = std::env::var("HOME").context("HOME not set")?;
    Ok(PathBuf::from(home)
        .join(".claude/projects")
        .join(encoded)
        .join(format!("{session_id}.jsonl")))
}

/// Check whether the last line of a JSONL file contains an assistant message
/// with the given message ID.
fn last_line_has_message_id(path: &Path, message_id: &str) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let mut reader = std::io::BufReader::new(file);

    // Seek near the end — session files can be large, we only need the last line.
    // 64KB is generous for a single JSONL line.
    let seek_pos = reader.seek(SeekFrom::End(0)).unwrap_or(0);
    let offset = seek_pos.saturating_sub(64 * 1024);
    let _ = reader.seek(SeekFrom::Start(offset));

    let mut last_line = String::new();
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        if !line.trim().is_empty() {
            std::mem::swap(&mut last_line, &mut line);
        }
        line.clear();
    }

    if last_line.is_empty() {
        return false;
    }

    // Minimal JSON parsing — check for the message ID string.
    // A full parse could fail on partial writes; a substring check is resilient.
    let target = format!("\"id\":\"{message_id}\"");
    let target_spaced = format!("\"id\": \"{message_id}\"");
    last_line.contains(&target) || last_line.contains(&target_spaced)
}

/// Wait for a Claude Code session file to contain the expected message,
/// using filesystem notifications with a timeout.
///
/// Returns `true` if the message was found, `false` on timeout.
fn wait_for_persist(working_dir: &Path, session_id: &str, message_id: &str) -> Result<bool> {
    let file_path = session_file_path(working_dir, session_id)?;

    // Check immediately — the file may already be up to date.
    if last_line_has_message_id(&file_path, message_id) {
        return Ok(true);
    }

    // Watch the file (or parent dir if file doesn't exist yet) for changes.
    let watch_path = if file_path.exists() {
        file_path.clone()
    } else if let Some(parent) = file_path.parent()
        && parent.exists()
    {
        parent.to_path_buf()
    } else {
        // Can't watch anything — give up.
        return Ok(false);
    };

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |_: notify::Result<notify::Event>| {
        let _ = tx.send(());
    })
    .context("failed to create filesystem watcher")?;
    watcher
        .watch(&watch_path, RecursiveMode::NonRecursive)
        .context("failed to watch session file")?;

    // Re-check after setting up watcher to avoid TOCTOU race.
    if last_line_has_message_id(&file_path, message_id) {
        return Ok(true);
    }

    let deadline = std::time::Instant::now() + PERSIST_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Ok(false);
        }
        match rx.recv_timeout(remaining) {
            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if last_line_has_message_id(&file_path, message_id) {
                    return Ok(true);
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Ok(false);
            }
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct WaitArgs {
    working_dir: String,
    session_id: String,
    message_id: String,
}

/// Wait for session file persistence if we have the IDs needed.
///
/// Checks `state.session_id` and `state.last_message_id`; if both are present,
/// waits for the session file to contain the expected message via VCR call.
pub async fn wait_if_needed(
    state: &super::state::SessionState,
    vcr: &VcrContext,
    working_dir: Option<&Path>,
) {
    if let Some(ref sid) = state.session_id
        && let Some(ref mid) = state.last_message_id
    {
        let dir = match working_dir {
            Some(d) => d.to_path_buf(),
            None => match std::env::current_dir() {
                Ok(d) => d,
                Err(_) => return,
            },
        };
        let _ = vcr
            .call(
                "wait_for_persist",
                WaitArgs {
                    working_dir: dir.display().to_string(),
                    session_id: sid.clone(),
                    message_id: mid.clone(),
                },
                async |a: &WaitArgs| {
                    wait_for_persist(Path::new(&a.working_dir), &a.session_id, &a.message_id)
                },
            )
            .await;
    }
}
