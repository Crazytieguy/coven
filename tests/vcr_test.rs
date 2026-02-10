#![allow(clippy::expect_used)]

use std::path::Path;

use coven::vcr::{Io, TestCase, VcrContext};

/// Strip ANSI escape codes for readable snapshots.
fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            while let Some(&next) = chars.peek() {
                chars.next();
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Run a test case through the real command function with VCR replay,
/// capturing renderer output for snapshot comparison.
async fn run_vcr_test(name: &str) -> String {
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

    // Default to haiku, matching what record-vcr uses during recording.
    let default_model = coven::vcr::DEFAULT_TEST_MODEL;

    if case.is_ralph() {
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
                extra_args,
                working_dir: None,
            },
            &mut io,
            &vcr,
            &mut output,
        )
        .await
        .expect("Command failed during VCR replay");
    } else {
        let run_config = case.run.as_ref().unwrap();
        let mut claude_args = run_config.claude_args.clone();
        if !claude_args.iter().any(|a| a == "--model") {
            claude_args.extend(["--model".to_string(), default_model.to_string()]);
        }
        coven::commands::run::run(
            Some(run_config.prompt.clone()),
            claude_args,
            case.display.show_thinking,
            None,
            &mut io,
            &vcr,
            &mut output,
        )
        .await
        .expect("Command failed during VCR replay");
    }

    let raw = String::from_utf8(output).expect("Output should be valid UTF-8");
    strip_ansi(&raw)
}

macro_rules! vcr_test {
    ($name:ident) => {
        #[tokio::test]
        async fn $name() {
            let clean = run_vcr_test(stringify!($name)).await;
            insta::with_settings!({
                snapshot_path => "../tests/cases",
                prepend_module_to_snapshot => false,
            }, {
                insta::assert_snapshot!(stringify!($name), clean);
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
