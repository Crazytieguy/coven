#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fmt::Write;
use std::path::{Path, PathBuf};

use coven::display::renderer::{StoredMessage, format_message};
use coven::vcr::{Io, MultiStep, TestCase, VcrContext};

/// Strip ANSI escape codes for readable snapshots.
fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek() {
                // CSI sequence: ESC [ ... <alpha>
                Some('[') => {
                    chars.next();
                    while let Some(&next) = chars.peek() {
                        chars.next();
                        if next.is_ascii_alphabetic() {
                            break;
                        }
                    }
                }
                // OSC sequence: ESC ] ... (BEL | ESC \)
                Some(']') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '\x07' {
                            break;
                        }
                        if next == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                // Other two-byte escape: ESC X
                Some(_) => {
                    chars.next();
                }
                // Trailing ESC at end of string
                None => {}
            }
        } else if c == '\r' {
            if chars.peek() == Some(&'\n') {
                // \r\n is a regular newline — just emit \n
                chars.next();
                result.push('\n');
            } else {
                // Bare \r — simulate carriage return: discard back to the last newline
                if let Some(pos) = result.rfind('\n') {
                    result.truncate(pos + 1);
                } else {
                    result.clear();
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Filter transient noise from snapshot output (e.g. rate limit warnings that
/// vary based on the recording user's current usage).
fn filter_snapshot_noise(s: &str) -> String {
    let mut result: String = s
        .lines()
        .filter(|line| !line.starts_with("[rate limit]"))
        .collect::<Vec<_>>()
        .join("\n");
    // Preserve trailing newline if the input had one (lines() strips it).
    if s.ends_with('\n') {
        result.push('\n');
    }
    result
}

struct TestResult {
    display: String,
    messages: Vec<StoredMessage>,
    views: Vec<String>,
}

/// Run a test case through the real command function with VCR replay,
/// capturing renderer output for snapshot comparison.
async fn run_vcr_test(theme: &str, name: &str) -> TestResult {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/cases")
        .join(theme)
        .join(name);
    let toml_path = base.join(format!("{name}.toml"));
    let vcr_path = base.join(format!("{name}.vcr"));

    let case: TestCase =
        toml::from_str(&std::fs::read_to_string(&toml_path).expect("Failed to read TOML file"))
            .expect("Failed to parse TOML file");

    let vcr_content = std::fs::read_to_string(&vcr_path).expect("Failed to read VCR file");
    let vcr = VcrContext::replay(&vcr_content).expect("Failed to parse VCR file");
    let mut io = Io::dummy();
    let mut output = Vec::new();
    let views = case.views.clone();

    // Default to haiku, matching what record-vcr uses during recording.
    let default_model = coven::vcr::DEFAULT_TEST_MODEL;

    let messages = if case.is_worker() {
        let worker_config = case.worker.as_ref().unwrap();
        let mut extra_args = worker_config.claude_args.clone();
        if !extra_args.iter().any(|a| a == "--model") {
            extra_args.extend(["--model".to_string(), default_model.to_string()]);
        }
        // Dummy path — never touched on disk since all worktree ops are VCR stubs during replay.
        let worktree_base = PathBuf::from("/tmp/coven-vcr-replay-worktrees");
        coven::commands::worker::worker(
            coven::commands::worker::WorkerConfig {
                show_thinking: case.display.show_thinking,
                branch: None,
                worktree_base,
                extra_args,
                working_dir: None,
                fork: false,
                reload: false,
                no_wait: false,
                term_width: Some(80),
            },
            &mut io,
            &vcr,
            &mut output,
        )
        .await
        .expect("Command failed during VCR replay");
        // Worker doesn't return StoredMessages; return empty vec.
        Vec::new()
    } else if case.is_ralph() {
        let ralph_config = case.ralph.as_ref().unwrap();
        let mut extra_args = ralph_config.claude_args.clone();
        if !extra_args.iter().any(|a| a == "--model") {
            extra_args.extend(["--model".to_string(), default_model.to_string()]);
        }
        coven::commands::ralph::ralph(
            coven::commands::ralph::RalphConfig {
                prompt: ralph_config.prompt.clone(),
                iterations: 10,
                break_tag: ralph_config.break_tag.clone(),
                no_break: false,
                no_wait: ralph_config.no_wait,
                show_thinking: case.display.show_thinking,
                tag_flags: coven::commands::ralph::TagFlags {
                    fork: false,
                    reload: false,
                },
                extra_args,
                working_dir: None,
                term_width: Some(80),
            },
            &mut io,
            &vcr,
            &mut output,
        )
        .await
        .expect("Command failed during VCR replay")
    } else if case.is_init() {
        let init_config = case.init.as_ref().unwrap();
        let stdin_input = format!("{}\n", init_config.stdin);
        let mut stdin = std::io::Cursor::new(stdin_input);
        coven::commands::init::init(&vcr, &mut output, &mut stdin, None)
            .await
            .expect("Command failed during VCR replay");
        Vec::new()
    } else if case.is_gc() {
        coven::commands::gc::gc(&vcr, false, None, &mut output)
            .await
            .expect("Command failed during VCR replay");
        Vec::new()
    } else if case.is_status() {
        coven::commands::status::status(&vcr, None, &mut output)
            .await
            .expect("Command failed during VCR replay");
        Vec::new()
    } else {
        let run_config = case.run.as_ref().unwrap();
        let mut claude_args = run_config.claude_args.clone();
        if !claude_args.iter().any(|a| a == "--model") {
            claude_args.extend(["--model".to_string(), default_model.to_string()]);
        }
        coven::commands::run::run(
            coven::commands::run::RunConfig {
                prompt: Some(run_config.prompt.clone()),
                extra_args: claude_args,
                show_thinking: case.display.show_thinking,
                fork: run_config.fork,
                reload: run_config.reload,
                working_dir: None,
                term_width: Some(80),
            },
            &mut io,
            &vcr,
            &mut output,
        )
        .await
        .expect("Command failed during VCR replay")
    };

    let raw = String::from_utf8(output).expect("Output should be valid UTF-8");
    TestResult {
        display: filter_snapshot_noise(&strip_ansi(&raw)),
        messages,
        views,
    }
}

/// Run a multi-step test case. Each step replays from its own VCR file,
/// and outputs are concatenated with `--- <step_name> ---` headers.
/// Steps sharing a `concurrent_group` run concurrently via `join_all`,
/// mirroring the recording-side behavior in `record_multi_case`.
async fn run_multi_vcr_test(theme: &str, name: &str) -> TestResult {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/cases")
        .join(theme)
        .join(name);
    let toml_path = base.join(format!("{name}.toml"));

    let case: TestCase =
        toml::from_str(&std::fs::read_to_string(&toml_path).expect("Failed to read TOML file"))
            .expect("Failed to parse TOML file");

    let multi = case.multi.expect("Expected multi config");
    let show_thinking = case.display.show_thinking;
    let default_model = coven::vcr::DEFAULT_TEST_MODEL;

    let mut combined_output = String::new();

    let mut steps = multi.steps.into_iter().peekable();
    while let Some(step) = steps.next() {
        if step.concurrent_group.is_some() {
            // Collect all consecutive steps with the same concurrent group.
            let group_name = step.concurrent_group.clone();
            let mut group = vec![step];
            while let Some(next) = steps.next_if(|s| s.concurrent_group == group_name) {
                group.push(next);
            }

            // Run grouped steps concurrently, mirroring the spawn_local behavior
            // used during recording. join_all polls futures in order, providing
            // deterministic interleaving at await points.
            let futures: Vec<_> = group
                .iter()
                .map(|step| async {
                    let vcr_path = base.join(format!("{name}__{}.vcr", step.name));
                    let vcr_content =
                        std::fs::read_to_string(&vcr_path).expect("Failed to read step VCR file");
                    let vcr =
                        VcrContext::replay(&vcr_content).expect("Failed to parse step VCR file");
                    let mut output = Vec::new();
                    run_multi_step(step, &vcr, show_thinking, default_model, &mut output).await;
                    let raw = String::from_utf8(output).expect("Output should be valid UTF-8");
                    (step.name.clone(), raw)
                })
                .collect();

            let results = futures::future::join_all(futures).await;
            for (step_name, raw) in results {
                combined_output.push_str(&format!("--- {step_name} ---\n"));
                combined_output.push_str(&filter_snapshot_noise(&strip_ansi(&raw)));
                combined_output.push('\n');
            }
        } else {
            // Sequential step (no concurrent group).
            let vcr_path = base.join(format!("{name}__{}.vcr", step.name));
            let vcr_content =
                std::fs::read_to_string(&vcr_path).expect("Failed to read step VCR file");
            let vcr = VcrContext::replay(&vcr_content).expect("Failed to parse step VCR file");
            let mut output = Vec::new();
            run_multi_step(&step, &vcr, show_thinking, default_model, &mut output).await;
            let raw = String::from_utf8(output).expect("Output should be valid UTF-8");
            combined_output.push_str(&format!("--- {} ---\n", step.name));
            combined_output.push_str(&filter_snapshot_noise(&strip_ansi(&raw)));
            combined_output.push('\n');
        }
    }

    TestResult {
        display: combined_output,
        messages: Vec::new(),
        views: Vec::new(),
    }
}

/// Replay a single step in a multi-step test case.
async fn run_multi_step(
    step: &MultiStep,
    vcr: &VcrContext,
    show_thinking: bool,
    default_model: &str,
    output: &mut Vec<u8>,
) {
    match step.command.as_str() {
        "init" => {
            let stdin_input = format!("{}\n", step.stdin.as_deref().unwrap_or(""));
            let mut stdin = std::io::Cursor::new(stdin_input);
            coven::commands::init::init(vcr, output, &mut stdin, None)
                .await
                .expect("Init step failed during VCR replay");
        }
        "worker" => {
            let mut io = Io::dummy();
            let mut extra_args = step.claude_args.clone();
            if !extra_args.iter().any(|a| a == "--model") {
                extra_args.extend(["--model".to_string(), default_model.to_string()]);
            }
            let worktree_base = PathBuf::from("/tmp/coven-vcr-replay-worktrees");
            coven::commands::worker::worker(
                coven::commands::worker::WorkerConfig {
                    show_thinking,
                    branch: None,
                    worktree_base,
                    extra_args,
                    working_dir: None,
                    fork: false,
                    reload: false,
                    no_wait: false,
                    term_width: Some(80),
                },
                &mut io,
                vcr,
                output,
            )
            .await
            .expect("Worker step failed during VCR replay");
        }
        other => panic!("unsupported multi-step command: {other}"),
    }
}

/// Format view output for snapshot: one section per viewed message.
fn format_views(messages: &[StoredMessage], views: &[String]) -> String {
    let mut out = String::new();
    for (i, query) in views.iter().enumerate() {
        if i > 0 {
            out.push_str("\n--- :next ---\n\n");
        }
        let _ = write!(out, ":{query}  ");
        let view = format_message(messages, query).expect("view query not found");
        out.push_str(&strip_ansi(&view));
        out.push('\n');
    }
    out
}

macro_rules! vcr_test {
    ($theme:ident / $name:ident) => {
        #[tokio::test]
        async fn $name() {
            let result = run_vcr_test(stringify!($theme), stringify!($name)).await;
            insta::with_settings!({
                snapshot_path => concat!("../tests/cases/", stringify!($theme), "/", stringify!($name)),
                prepend_module_to_snapshot => false,
            }, {
                insta::assert_snapshot!(stringify!($name), result.display);
            });
            if !result.views.is_empty() {
                let views = format_views(&result.messages, &result.views);
                insta::with_settings!({
                    snapshot_path => concat!("../tests/cases/", stringify!($theme), "/", stringify!($name)),
                    prepend_module_to_snapshot => false,
                }, {
                    insta::assert_snapshot!(concat!(stringify!($name), "__views"), views);
                });
            }
        }
    };
}

macro_rules! multi_vcr_test {
    ($theme:ident / $name:ident) => {
        #[tokio::test]
        async fn $name() {
            let result = run_multi_vcr_test(stringify!($theme), stringify!($name)).await;
            insta::with_settings!({
                snapshot_path => concat!("../tests/cases/", stringify!($theme), "/", stringify!($name)),
                prepend_module_to_snapshot => false,
            }, {
                insta::assert_snapshot!(stringify!($name), result.display);
            });
        }
    };
}

// Session: core session lifecycle
vcr_test!(session / simple_qa);
vcr_test!(session / multi_turn);
vcr_test!(session / steering);
vcr_test!(session / interrupt_resume);
vcr_test!(session / show_thinking);
vcr_test!(session / error_handling);
vcr_test!(session / reload_basic);

// Rendering: tool output display
vcr_test!(rendering / tool_use);
vcr_test!(rendering / grep_glob);
vcr_test!(rendering / mcp_tool);
vcr_test!(rendering / edit_tool);
vcr_test!(rendering / write_single_line);

// Subagent: spawning and error handling
vcr_test!(subagent / subagent);
vcr_test!(subagent / parallel_subagent);
vcr_test!(subagent / subagent_error);

// Fork: parallel sub-sessions
vcr_test!(fork / fork_basic);
vcr_test!(fork / fork_buffered);
vcr_test!(fork / fork_single);

// Ralph: loop mode
vcr_test!(ralph / ralph_break);
vcr_test!(ralph / ralph_no_wait);
vcr_test!(ralph / ralph_continue);
vcr_test!(ralph / ralph_immediate_break);

// Orchestration: worker, init, status, gc
vcr_test!(orchestration / worker_basic);
vcr_test!(orchestration / init_fresh);
vcr_test!(orchestration / status_no_workers);
vcr_test!(orchestration / gc_no_orphans);
multi_vcr_test!(orchestration / concurrent_workers);
multi_vcr_test!(orchestration / ambiguous_task);
multi_vcr_test!(orchestration / priority_dispatch);
multi_vcr_test!(orchestration / landing_conflict);
