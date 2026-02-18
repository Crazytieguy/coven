use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tokio::sync::{Semaphore, mpsc};
use tokio::task::LocalSet;

use coven::commands;
use coven::vcr::{DEFAULT_TEST_MODEL, Io, MultiStep, TestCase, TriggerController, VcrContext};

/// Writes to stderr with a `[prefix] ` prepended to each line.
struct PrefixWriter {
    prefix: String,
    stderr: std::io::Stderr,
    at_line_start: bool,
}

impl PrefixWriter {
    fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            stderr: std::io::stderr(),
            at_line_start: true,
        }
    }
}

impl Write for PrefixWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = buf.len();
        let mut start = 0;
        for (i, &byte) in buf.iter().enumerate() {
            if self.at_line_start {
                write!(self.stderr, "[{}] ", self.prefix)?;
                self.at_line_start = false;
            }
            if byte == b'\n' {
                self.stderr.write_all(&buf[start..=i])?;
                start = i + 1;
                self.at_line_start = true;
            }
        }
        if start < len {
            self.stderr.write_all(&buf[start..])?;
        }
        Ok(len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.stderr.flush()
    }
}

/// A discovered test case: its directory and name.
struct CaseEntry {
    /// Directory containing the case files (e.g. `tests/cases/session/simple_qa/`).
    case_dir: PathBuf,
    /// The test case name (e.g. `simple_qa`).
    name: String,
}

/// Discover all test cases by walking `tests/cases/{theme}/{name}/{name}.toml`.
fn discover_cases(cases_dir: &Path) -> Result<Vec<CaseEntry>> {
    let mut entries = Vec::new();
    let mut themes: Vec<_> = std::fs::read_dir(cases_dir)?
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_ok_and(|ft| ft.is_dir()))
        .collect();
    themes.sort_by_key(std::fs::DirEntry::path);

    for theme in themes {
        let mut names: Vec<_> = std::fs::read_dir(theme.path())?
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_type().is_ok_and(|ft| ft.is_dir()))
            .collect();
        names.sort_by_key(std::fs::DirEntry::path);

        for name_entry in names {
            let name = name_entry
                .file_name()
                .to_str()
                .map(String::from)
                .context("non-UTF8 directory name")?;
            let toml_path = name_entry.path().join(format!("{name}.toml"));
            if toml_path.exists() {
                entries.push(CaseEntry {
                    case_dir: name_entry.path(),
                    name,
                });
            }
        }
    }
    Ok(entries)
}

#[tokio::main]
async fn main() -> Result<()> {
    const MAX_CONCURRENT: usize = 8;

    let args: Vec<String> = std::env::args().collect();
    let cases_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/cases");

    let all_cases = discover_cases(&cases_dir)?;

    let cases: Vec<CaseEntry> = if args.len() > 1 {
        // Filter by CLI args: accept "name" (searches all themes) or "theme/name".
        let filters: Vec<&str> = args[1..].iter().map(String::as_str).collect();
        let mut matched = Vec::new();
        for filter in &filters {
            let found: Vec<_> = if filter.contains('/') {
                // theme/name form
                let parts: Vec<&str> = filter.splitn(2, '/').collect();
                all_cases
                    .iter()
                    .filter(|c| {
                        c.case_dir
                            .parent()
                            .and_then(|p| p.file_name())
                            .is_some_and(|t| t == parts[0])
                            && c.name == parts[1]
                    })
                    .collect()
            } else {
                // name only — search all themes
                all_cases.iter().filter(|c| c.name == *filter).collect()
            };
            if found.is_empty() {
                bail!("no test case found matching '{filter}'");
            }
            for entry in found {
                matched.push(CaseEntry {
                    case_dir: entry.case_dir.clone(),
                    name: entry.name.clone(),
                });
            }
        }
        matched
    } else {
        all_cases
    };

    // Record cases concurrently using LocalSet (VcrContext is !Send due to RefCell).
    // Tasks interleave at await points — the real parallelism is I/O-bound (Claude API calls).
    // Cap concurrency at 8 to avoid overwhelming the API.
    let local = LocalSet::new();
    let errors = local
        .run_until(async {
            let semaphore = std::rc::Rc::new(Semaphore::new(MAX_CONCURRENT));
            let mut handles = Vec::new();
            for case in cases {
                let sem = semaphore.clone();
                handles.push(tokio::task::spawn_local(async move {
                    let name = case.name.clone();
                    let result = async {
                        let _permit = sem.acquire().await.map_err(|e| anyhow::anyhow!("{e}"))?;
                        record_case(&case.case_dir, &case.name).await
                    }
                    .await;
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

/// Stage all changes and commit with the given message.
fn git_commit_all(dir: &Path, message: &str) -> Result<()> {
    for (cmd, args) in [("add", vec!["."]), ("commit", vec!["-m", message])] {
        let output = std::process::Command::new("git")
            .arg(cmd)
            .args(&args)
            .current_dir(dir)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "git {cmd} failed: {}",
            String::from_utf8_lossy(&output.stderr)
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
        ("config", vec!["user.name", "test"]),
        ("config", vec!["user.email", "test@test.com"]),
        ("add", vec!["."]),
        ("commit", vec!["-m", "initial", "--allow-empty"]),
    ] {
        let output = std::process::Command::new("git")
            .arg(cmd)
            .args(&args)
            .current_dir(&tmp_dir)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "git {cmd} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(tmp_dir)
}

/// Ensure `--model` is present in extra args, defaulting to `DEFAULT_TEST_MODEL`.
fn ensure_model_arg(args: &mut Vec<String>) {
    if !args.iter().any(|a| a == "--model") {
        args.extend(["--model".to_string(), DEFAULT_TEST_MODEL.to_string()]);
    }
}

async fn record_case(case_dir: &Path, name: &str) -> Result<()> {
    let toml_path = case_dir.join(format!("{name}.toml"));
    let vcr_path = case_dir.join(format!("{name}.vcr"));

    let toml_content = std::fs::read_to_string(&toml_path)
        .context(format!("Failed to read {}", toml_path.display()))?;
    let case: TestCase = toml::from_str(&toml_content)?;

    if case.is_multi() {
        return record_multi_case(case_dir, name, case).await;
    }

    let tmp_dir = setup_test_dir(name, &case)?;
    let (term_tx, term_rx) = mpsc::unbounded_channel();
    let (_event_tx, event_rx) = mpsc::unbounded_channel();
    let controller = TriggerController::new(&case.messages, term_tx)?.with_auto_exit();
    let vcr = VcrContext::record_with_triggers(controller);
    let mut io = Io::new(event_rx, term_rx);
    let mut output = PrefixWriter::new(name);

    if case.is_worker() {
        let worker_config = case.worker.as_ref().context("worker config missing")?;
        let mut extra_args = worker_config.claude_args.clone();
        ensure_model_arg(&mut extra_args);
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
                reload: false,
                term_width: Some(80),
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
        ensure_model_arg(&mut extra_args);
        commands::ralph::ralph(
            commands::ralph::RalphConfig {
                prompt: ralph_config.prompt.clone(),
                iterations: 10, // safety limit for recording
                break_tag: ralph_config.break_tag.clone(),
                no_break: false,
                show_thinking: case.display.show_thinking,
                tag_flags: commands::ralph::TagFlags {
                    fork: false,
                    reload: false,
                },
                extra_args,
                working_dir: Some(tmp_dir.clone()),
                term_width: Some(80),
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
        commands::gc::gc(&vcr, false, Some(tmp_dir.as_path()), &mut output).await?;
    } else if case.is_status() {
        commands::status::status(&vcr, Some(tmp_dir.as_path()), &mut output).await?;
    } else {
        let run_config = case.run.as_ref().context("run config missing")?;
        let mut claude_args = run_config.claude_args.clone();
        ensure_model_arg(&mut claude_args);
        commands::run::run(
            commands::run::RunConfig {
                prompt: Some(run_config.prompt.clone()),
                extra_args: claude_args,
                show_thinking: case.display.show_thinking,
                fork: run_config.fork,
                reload: false,
                working_dir: Some(tmp_dir.clone()),
                term_width: Some(80),
            },
            &mut io,
            &vcr,
            &mut output,
        )
        .await?;
    }

    vcr.write_recording(&vcr_path)?;
    std::fs::remove_dir_all(&tmp_dir).ok();
    Ok(())
}

/// Record a multi-step test case. Steps are executed sequentially unless they
/// share a `concurrent_group`, in which case they run concurrently.
/// Each step writes its own VCR file: `<test>__<step>.vcr`.
async fn record_multi_case(case_dir: &Path, name: &str, case: TestCase) -> Result<()> {
    let tmp_dir = setup_test_dir(name, &case)?;
    let show_thinking = case.display.show_thinking;
    let multi = case
        .multi
        .context("record_multi_case called without multi config")?;

    let mut steps = multi.steps.into_iter().peekable();
    while let Some(step) = steps.next() {
        if step.concurrent_group.is_some() {
            let group_name = step.concurrent_group.clone();
            let mut group = vec![step];
            while let Some(next_step) = steps.next_if(|s| s.concurrent_group == group_name) {
                group.push(next_step);
            }
            let mut handles = Vec::new();
            for step in group {
                let dir = case_dir.to_path_buf();
                let n = name.to_string();
                let td = tmp_dir.clone();
                handles.push(tokio::task::spawn_local(async move {
                    record_multi_step(dir, n, step, td, show_thinking).await
                }));
            }
            for handle in handles {
                handle.await??;
            }
        } else {
            record_multi_step(
                case_dir.to_path_buf(),
                name.to_string(),
                step,
                tmp_dir.clone(),
                show_thinking,
            )
            .await?;
        }
    }

    std::fs::remove_dir_all(&tmp_dir).ok();
    Ok(())
}

/// Record a single step in a multi-step test case.
async fn record_multi_step(
    case_dir: PathBuf,
    test_name: String,
    step: MultiStep,
    tmp_dir: PathBuf,
    show_thinking: bool,
) -> Result<()> {
    let vcr_path = case_dir.join(format!("{test_name}__{}.vcr", step.name));
    let default_model = DEFAULT_TEST_MODEL;

    match step.command.as_str() {
        "init" => {
            let vcr = VcrContext::record();
            let mut output = PrefixWriter::new(format!("{test_name}/{}", step.name));
            let stdin_input = format!("{}\n", step.stdin.as_deref().unwrap_or(""));
            let mut stdin = std::io::Cursor::new(stdin_input);
            commands::init::init(&vcr, &mut output, &mut stdin, Some(tmp_dir.clone())).await?;
            vcr.write_recording(&vcr_path)?;

            // Commit init-created files so they're available in worktree checkouts.
            git_commit_all(&tmp_dir, "coven init")?;
        }
        "worker" => {
            let (term_tx, term_rx) = mpsc::unbounded_channel();
            let (_event_tx, event_rx) = mpsc::unbounded_channel();

            let controller = TriggerController::new(&step.messages, term_tx)?.with_auto_exit();
            let vcr = VcrContext::record_with_triggers(controller);
            let mut io = Io::new(event_rx, term_rx);
            let mut output = PrefixWriter::new(format!("{test_name}/{}", step.name));

            let mut extra_args = step.claude_args;
            if !extra_args.iter().any(|a| a == "--model") {
                extra_args.extend(["--model".to_string(), default_model.to_string()]);
            }

            let worktree_base =
                tmp_dir.with_file_name(format!("coven-vcr-{test_name}-{}-worktrees", step.name));
            std::fs::create_dir_all(&worktree_base)?;

            commands::worker::worker(
                commands::worker::WorkerConfig {
                    show_thinking,
                    branch: None,
                    worktree_base: worktree_base.clone(),
                    extra_args,
                    working_dir: Some(tmp_dir),
                    fork: false,
                    reload: false,
                    term_width: Some(80),
                },
                &mut io,
                &vcr,
                &mut output,
            )
            .await?;

            vcr.write_recording(&vcr_path)?;
            std::fs::remove_dir_all(&worktree_base).ok();
        }
        other => bail!("unsupported multi-step command: {other}"),
    }

    Ok(())
}
