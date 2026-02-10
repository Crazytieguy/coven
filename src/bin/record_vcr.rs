use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use tokio::task::LocalSet;

use coven::commands;
use coven::vcr::{DEFAULT_TEST_MODEL, Io, TestCase, TriggerController, VcrContext};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cases_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/cases");

    let names: Vec<String> = if args.len() > 1 {
        args[1..].to_vec()
    } else {
        let mut entries: Vec<_> = std::fs::read_dir(&cases_dir)?
            .filter_map(std::result::Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
            .collect();
        entries.sort_by_key(std::fs::DirEntry::path);
        entries
            .iter()
            .filter_map(|e| {
                e.path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(String::from)
            })
            .collect()
    };

    // Record all cases concurrently using LocalSet (VcrContext is !Send due to RefCell).
    // Tasks interleave at await points — the real parallelism is I/O-bound (Claude API calls).
    let local = LocalSet::new();
    let errors = local
        .run_until(async {
            let mut handles = Vec::new();
            for name in names {
                let dir = cases_dir.clone();
                handles.push(tokio::task::spawn_local(async move {
                    let result = record_case(&dir, &name).await;
                    (name, result)
                }));
            }

            let mut errors = Vec::new();
            for handle in handles {
                match handle.await {
                    Ok((name, Ok(()))) => eprintln!("  Done: {name}.vcr"),
                    Ok((name, Err(e))) => {
                        eprintln!("  FAILED: {name}: {e}");
                        errors.push((name, e));
                    }
                    Err(e) => {
                        eprintln!("  FAILED: task panicked: {e}");
                        errors.push(("(panicked)".to_string(), e.into()));
                    }
                }
            }
            errors
        })
        .await;

    if !errors.is_empty() {
        anyhow::bail!(
            "{} recording(s) failed: {}",
            errors.len(),
            errors
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
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

    // Initialize git repo with initial commit for test environment.
    let git_init = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&tmp_dir)
        .output()?;
    anyhow::ensure!(
        git_init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&git_init.stderr)
    );

    let git_add = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&tmp_dir)
        .output()?;
    anyhow::ensure!(
        git_add.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&git_add.stderr)
    );

    let git_commit = std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&tmp_dir)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@test.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@test.com")
        .output()?;
    anyhow::ensure!(
        git_commit.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&git_commit.stderr)
    );

    // Set up VCR recording with trigger controller
    let (term_tx, term_rx) = mpsc::unbounded_channel();
    let (_event_tx, event_rx) = mpsc::unbounded_channel();

    let mut controller = TriggerController::new(&case.messages, term_tx);
    if !case.is_ralph() {
        controller = controller.with_auto_exit();
    }
    let vcr = VcrContext::record_with_triggers(controller);
    let mut io = Io::new(event_rx, term_rx);

    // Run the real command function.
    // Default to haiku for recording unless the test case specifies a model.
    let default_model = DEFAULT_TEST_MODEL;

    // Capture output to a buffer (stdout would interleave in parallel).
    let mut output = Vec::new();

    if case.is_ralph() {
        let ralph_config = case.ralph.as_ref().context("ralph config missing")?;
        let mut extra_args = ralph_config.claude_args.clone();
        if !extra_args.iter().any(|a| a == "--model") {
            extra_args.extend(["--model".to_string(), default_model.to_string()]);
        }
        commands::ralph::ralph(
            commands::ralph::RalphConfig {
                prompt: ralph_config.prompt.clone(),
                iterations: 10, // safety limit for recording
                break_tag: ralph_config.break_tag.clone(),
                no_break: false,
                show_thinking: case.display.show_thinking,
                extra_args,
                working_dir: Some(tmp_dir.clone()),
            },
            &mut io,
            &vcr,
            &mut output,
        )
        .await?;
    } else {
        let run_config = case.run.as_ref().context("run config missing")?;
        let mut claude_args = run_config.claude_args.clone();
        if !claude_args.iter().any(|a| a == "--model") {
            claude_args.extend(["--model".to_string(), default_model.to_string()]);
        }
        commands::run::run(
            Some(run_config.prompt.clone()),
            claude_args,
            case.display.show_thinking,
            Some(tmp_dir.clone()),
            &mut io,
            &vcr,
            &mut output,
        )
        .await?;
    }

    // Write the VCR recording
    vcr.write_recording(&vcr_path)?;

    // Clean up temp dir
    std::fs::remove_dir_all(&tmp_dir).ok();

    Ok(())
}
