use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use coven::protocol::emit::format_user_message;
use coven::protocol::parse::parse_line;
use coven::protocol::types::InboundEvent;
use coven::vcr::{TestCase, Trigger, VcrHeader};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cases_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/cases");

    if args.len() > 1 {
        for name in &args[1..] {
            eprintln!("Recording: {name}");
            record_case(&cases_dir, name).await?;
            eprintln!("  Done: {name}.vcr");
        }
    } else {
        // Record all cases
        let mut entries: Vec<_> = std::fs::read_dir(&cases_dir)?
            .filter_map(std::result::Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
            .collect();
        entries.sort_by_key(std::fs::DirEntry::path);

        for entry in entries {
            let name = entry
                .path()
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            eprintln!("Recording: {name}");
            record_case(&cases_dir, &name).await?;
            eprintln!("  Done: {name}.vcr");
        }
    }

    Ok(())
}

async fn record_case(cases_dir: &Path, name: &str) -> Result<()> {
    let toml_path = cases_dir.join(format!("{name}.toml"));
    let vcr_path = cases_dir.join(format!("{name}.vcr"));

    let toml_content = std::fs::read_to_string(&toml_path)
        .context(format!("Failed to read {}", toml_path.display()))?;
    let case: TestCase = toml::from_str(&toml_content)?;

    // Set up temp working directory
    let tmp_dir = std::env::temp_dir().join(format!("coven-vcr-{name}"));
    if tmp_dir.exists() {
        std::fs::remove_dir_all(&tmp_dir)?;
    }
    std::fs::create_dir_all(&tmp_dir)?;

    for (path, content) in &case.files {
        let file_path = tmp_dir.join(path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, content)?;
    }

    // Create .claude/settings.json with broad permissions so recordings
    // aren't blocked by permission prompts.
    let claude_dir = tmp_dir.join(".claude");
    std::fs::create_dir_all(&claude_dir)?;
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{"permissions":{"allow":["Bash(*)","WebFetch","WebSearch","mcp__plugin_llms-fetch-mcp_llms-fetch__fetch"]}}"#,
    )?;

    // Build VCR header
    let expected_command = case.expected_command();
    let header = VcrHeader {
        vcr: "header".to_string(),
        command: expected_command.clone(),
    };
    let header_line = serde_json::to_string(&header)?;

    let mut vcr_lines = vec![header_line];

    if case.is_ralph() {
        record_ralph(&case, &expected_command, &tmp_dir, &mut vcr_lines).await?;
    } else {
        record_run(&case, &expected_command, &tmp_dir, &mut vcr_lines).await?;
    }

    // Write VCR file
    let vcr_content = vcr_lines.join("\n") + "\n";
    std::fs::write(&vcr_path, vcr_content)?;

    // Clean up temp dir
    std::fs::remove_dir_all(&tmp_dir).ok();

    Ok(())
}

/// Record a standard run session.
async fn record_run(
    case: &TestCase,
    command: &[String],
    work_dir: &Path,
    vcr_lines: &mut Vec<String>,
) -> Result<()> {
    // Spawn claude
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(work_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to spawn claude")?;

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    // Send initial prompt
    let prompt_msg = format_user_message(case.prompt());
    stdin.write_all(prompt_msg.as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await?;
    vcr_lines.push(format!("> {prompt_msg}"));

    // Parse triggers from messages
    let pending_messages: Vec<_> = case
        .messages
        .iter()
        .filter_map(|m| Trigger::parse(&m.trigger).map(|t| (t, m.content.clone())))
        .collect();

    // Track which messages have been sent
    let mut sent = vec![false; pending_messages.len()];
    let mut tool_count: usize = 0;
    let mut message_count: usize = 0;
    let mut got_result = false;

    // Read stdout
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        vcr_lines.push(format!("< {line}"));

        // Parse to detect triggers
        if let Ok(Some(event)) = parse_line(&line) {
            match &event {
                InboundEvent::User(u) if u.tool_use_result.is_some() => {
                    tool_count += 1;
                }
                InboundEvent::Assistant(_) => {
                    message_count += 1;
                }
                InboundEvent::Result(_) => {
                    got_result = true;
                }
                _ => {}
            }

            // Check if any pending message should fire
            for (i, (trigger, content)) in pending_messages.iter().enumerate() {
                if !sent[i] && trigger.fires(tool_count, message_count, got_result) {
                    let msg = format_user_message(content);
                    stdin.write_all(msg.as_bytes()).await?;
                    stdin.write_all(b"\n").await?;
                    stdin.flush().await?;
                    vcr_lines.push(format!("> {msg}"));
                    sent[i] = true;
                    // Reset for next trigger
                    got_result = false;
                }
            }

            // If we got a result and all messages are sent (and none
            // were just fired on this very result), we're done.
            // Use `got_result` rather than pattern-matching the event so
            // that a trigger which fires on after-result and resets
            // `got_result` prevents an immediate break.
            if got_result && sent.iter().all(|&s| s) {
                break;
            }
        }
    }

    // Close stdin and wait
    drop(stdin);
    child.wait().await?;

    Ok(())
}

/// Record a ralph loop session.
async fn record_ralph(
    case: &TestCase,
    command: &[String],
    work_dir: &Path,
    vcr_lines: &mut Vec<String>,
) -> Result<()> {
    let break_tag = case.break_tag().unwrap_or("break");
    let max_iterations = 10; // safety limit for recording

    for iteration in 0..max_iterations {
        if iteration > 0 {
            vcr_lines.push("---".to_string());
        }

        // Spawn fresh claude for each iteration
        let mut child = Command::new(&command[0])
            .args(&command[1..])
            .current_dir(work_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to spawn claude")?;

        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        // Send prompt
        let prompt_msg = format_user_message(case.prompt());
        stdin.write_all(prompt_msg.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        vcr_lines.push(format!("> {prompt_msg}"));

        // Read stdout
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        let mut result_text = String::new();

        while let Ok(Some(line)) = lines.next_line().await {
            vcr_lines.push(format!("< {line}"));

            if let Ok(Some(InboundEvent::Result(ref result))) = parse_line(&line) {
                result_text.clone_from(&result.result);
                break;
            }
        }

        // Close stdin and wait
        drop(stdin);
        child.wait().await?;

        // Check for break tag
        let open = format!("<{break_tag}>");
        let close = format!("</{break_tag}>");
        if result_text.contains(&open) && result_text.contains(&close) {
            eprintln!("  Break tag detected at iteration {}", iteration + 1);
            break;
        }
    }

    Ok(())
}
