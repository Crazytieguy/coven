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
        } else {
            result.push(c);
        }
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
async fn run_vcr_test(name: &str) -> TestResult {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
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
                show_thinking: case.display.show_thinking,
                fork: false,
                extra_args,
                working_dir: None,
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
        coven::commands::gc::gc(&vcr, None, &mut output)
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
                working_dir: None,
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
        display: strip_ansi(&raw),
        messages,
        views,
    }
}

/// Run a multi-step test case. Each step replays from its own VCR file,
/// and outputs are concatenated with `--- <step_name> ---` headers.
async fn run_multi_vcr_test(name: &str) -> TestResult {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let toml_path = base.join(format!("{name}.toml"));

    let case: TestCase =
        toml::from_str(&std::fs::read_to_string(&toml_path).expect("Failed to read TOML file"))
            .expect("Failed to parse TOML file");

    let multi = case.multi.as_ref().expect("Expected multi config");
    let show_thinking = case.display.show_thinking;
    let default_model = coven::vcr::DEFAULT_TEST_MODEL;

    let mut combined_output = String::new();

    for step in &multi.steps {
        let vcr_path = base.join(format!("{name}__{}.vcr", step.name));
        let vcr_content = std::fs::read_to_string(&vcr_path).expect("Failed to read step VCR file");
        let vcr = VcrContext::replay(&vcr_content).expect("Failed to parse step VCR file");

        let mut output = Vec::new();
        run_multi_step(step, &vcr, show_thinking, default_model, &mut output).await;

        let raw = String::from_utf8(output).expect("Output should be valid UTF-8");
        combined_output.push_str(&format!("--- {} ---\n", step.name));
        combined_output.push_str(&strip_ansi(&raw));
        combined_output.push('\n');
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
    ($name:ident) => {
        #[tokio::test]
        async fn $name() {
            let result = run_vcr_test(stringify!($name)).await;
            insta::with_settings!({
                snapshot_path => "../tests/cases",
                prepend_module_to_snapshot => false,
            }, {
                insta::assert_snapshot!(stringify!($name), result.display);
            });
            if !result.views.is_empty() {
                let views = format_views(&result.messages, &result.views);
                insta::with_settings!({
                    snapshot_path => "../tests/cases",
                    prepend_module_to_snapshot => false,
                }, {
                    insta::assert_snapshot!(concat!(stringify!($name), "__views"), views);
                });
            }
        }
    };
}

macro_rules! multi_vcr_test {
    ($name:ident) => {
        #[tokio::test]
        async fn $name() {
            let result = run_multi_vcr_test(stringify!($name)).await;
            insta::with_settings!({
                snapshot_path => "../tests/cases",
                prepend_module_to_snapshot => false,
            }, {
                insta::assert_snapshot!(stringify!($name), result.display);
            });
        }
    };
}

vcr_test!(simple_qa);
vcr_test!(tool_use);
vcr_test!(grep_glob);
vcr_test!(mcp_tool);
vcr_test!(error_handling);
vcr_test!(multi_turn);
vcr_test!(ralph_break);
vcr_test!(steering);
vcr_test!(subagent);
vcr_test!(write_single_line);
vcr_test!(edit_tool);
vcr_test!(show_thinking);
vcr_test!(subagent_error);
vcr_test!(parallel_subagent);
vcr_test!(worker_basic);
vcr_test!(interrupt_resume);
vcr_test!(status_no_workers);
vcr_test!(gc_no_orphans);
vcr_test!(init_fresh);
vcr_test!(fork_basic);
vcr_test!(fork_buffered);
vcr_test!(fork_single);
multi_vcr_test!(concurrent_workers);
