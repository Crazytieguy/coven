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

/// Create a temp directory with test files and an initial git commit.
fn setup_test_dir(name: &str, case: &TestCase) -> Result<PathBuf> {
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

    for (cmd, args) in [
        ("init", vec![]),
        ("add", vec!["."]),
        ("commit", vec!["-m", "initial", "--allow-empty"]),
    ] {
        let output = std::process::Command::new("git")
            .arg(cmd)
            .args(&args)
            .current_dir(&tmp_dir)
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "git {cmd} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(tmp_dir)
}

async fn record_case(cases_dir: &Path, name: &str) -> Result<()> {
    let toml_path = cases_dir.join(format!("{name}.toml"));
    let vcr_path = cases_dir.join(format!("{name}.vcr"));

    let toml_content = std::fs::read_to_string(&toml_path)
        .context(format!("Failed to read {}", toml_path.display()))?;
    let case: TestCase = toml::from_str(&toml_content)?;

    let tmp_dir = setup_test_dir(name, &case)?;

    // Set up VCR recording with trigger controller
    let (term_tx, term_rx) = mpsc::unbounded_channel();
    let (_event_tx, event_rx) = mpsc::unbounded_channel();

    let mut controller = TriggerController::new(&case.messages, term_tx)?;
    // Auto-exit for run mode: inject Ctrl+D after all triggers fire and result is seen.
    // Ralph and worker modes handle exit differently (break tag / explicit exit trigger).
    if !case.is_ralph() && !case.is_worker() {
        controller = controller.with_auto_exit();
    }
    let vcr = VcrContext::record_with_triggers(controller);
    let mut io = Io::new(event_rx, term_rx);

    // Run the real command function.
    // Default to haiku for recording unless the test case specifies a model.
    let default_model = DEFAULT_TEST_MODEL;

    // Capture output to a buffer (stdout would interleave in parallel).
    let mut output = Vec::new();

    if case.is_worker() {
        let worker_config = case.worker.as_ref().context("worker config missing")?;
        let mut extra_args = worker_config.claude_args.clone();
        if !extra_args.iter().any(|a| a == "--model") {
            extra_args.extend(["--model".to_string(), default_model.to_string()]);
        }
        // Worker needs a worktree base directory (sibling of the test repo).
        let worktree_base = tmp_dir.with_file_name(format!("coven-vcr-{name}-worktrees"));
        std::fs::create_dir_all(&worktree_base)?;
        commands::worker::worker(
            commands::worker::WorkerConfig {
                show_thinking: case.display.show_thinking,
                branch: None,
                worktree_base: worktree_base.clone(),
                extra_args,
                working_dir: Some(tmp_dir.clone()),
                fork: false,
            },
            &mut io,
            &vcr,
            &mut output,
        )
        .await?;
        // Clean up worktree base
        std::fs::remove_dir_all(&worktree_base).ok();
    } else if case.is_ralph() {
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
                fork: false,
                extra_args,
                working_dir: Some(tmp_dir.clone()),
            },
            &mut io,
            &vcr,
            &mut output,
        )
        .await?;
    } else if case.is_init() {
        let init_config = case.init.as_ref().context("init config missing")?;
        let stdin_input = format!("{}\n", init_config.stdin);
        let mut stdin = std::io::Cursor::new(stdin_input);
        commands::init::init(&vcr, &mut output, &mut stdin, Some(tmp_dir.clone())).await?;
    } else if case.is_gc() {
        commands::gc::gc(&vcr, Some(tmp_dir.as_path()), &mut output).await?;
    } else if case.is_status() {
        commands::status::status(&vcr, Some(tmp_dir.as_path()), &mut output).await?;
    } else {
        let run_config = case.run.as_ref().context("run config missing")?;
        let mut claude_args = run_config.claude_args.clone();
        if !claude_args.iter().any(|a| a == "--model") {
            claude_args.extend(["--model".to_string(), default_model.to_string()]);
        }
        commands::run::run(
            commands::run::RunConfig {
                prompt: Some(run_config.prompt.clone()),
                extra_args: claude_args,
                show_thinking: case.display.show_thinking,
                fork: false,
                working_dir: Some(tmp_dir.clone()),
            },
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
