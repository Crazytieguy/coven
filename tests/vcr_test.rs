#![allow(clippy::expect_used)]

use std::path::Path;

use coven::display::renderer::Renderer;
use coven::handle_inbound;
use coven::protocol::emit::format_user_message;
use coven::protocol::parse::parse_line;
use coven::session::state::SessionState;
use coven::vcr::{DisplayConfig, TestCase, VcrHeader};

// --- VCR validation ---

/// Validate VCR header CLI args and stdin lines against the test case definition.
fn validate_vcr(case: &TestCase, vcr_lines: &[&str]) {
    // 1. Validate header
    let header: VcrHeader =
        serde_json::from_str(vcr_lines[0]).expect("First line should be valid VCR header JSON");
    assert_eq!(header.vcr, "header");

    let expected_command = case
        .expected_command()
        .expect("Test case should have [run] or [ralph]");
    assert_eq!(
        header.command, expected_command,
        "VCR header CLI args mismatch"
    );

    // 2. Validate stdin lines match TOML messages
    let stdin_lines: Vec<&str> = vcr_lines[1..]
        .iter()
        .filter_map(|l| l.strip_prefix("> "))
        .collect();

    if case.is_ralph() {
        // Ralph: same message repeated each iteration
        let iterations = vcr_lines[1..].iter().filter(|l| l.trim() == "---").count() + 1;
        assert_eq!(
            stdin_lines.len(),
            iterations,
            "Ralph VCR should have one stdin line per iteration"
        );
        let expected_msg = format_user_message(case.prompt().expect("should have prompt"))
            .expect("serialization should succeed");
        for (i, stdin_line) in stdin_lines.iter().enumerate() {
            assert_eq!(
                *stdin_line, expected_msg,
                "Ralph stdin line {i} doesn't match expected prompt"
            );
        }
    } else {
        // Build expected stdin messages: initial prompt + follow-up messages
        let mut expected: Vec<String> = vec![
            format_user_message(case.prompt().expect("should have prompt"))
                .expect("serialization should succeed"),
        ];
        for msg in &case.messages {
            expected.push(format_user_message(&msg.content).expect("serialization should succeed"));
        }

        assert_eq!(
            stdin_lines.len(),
            expected.len(),
            "Number of stdin lines ({}) doesn't match expected ({})",
            stdin_lines.len(),
            expected.len()
        );
        for (i, (actual, expected_msg)) in stdin_lines.iter().zip(expected.iter()).enumerate() {
            assert_eq!(*actual, expected_msg.as_str(), "Stdin line {i} mismatch");
        }
    }
}

// --- VCR replay ---

/// Replay VCR stdout lines through the renderer, capturing output.
fn replay_stdout(vcr_lines: &[&str], display: &DisplayConfig) -> String {
    let mut output = Vec::new();
    let mut renderer = Renderer::with_writer(&mut output);
    renderer.set_show_thinking(display.show_thinking);
    let mut state = SessionState::default();
    let mut seen_first_stdin = false;

    for line in &vcr_lines[1..] {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // `---` separates ralph iterations
        if line == "---" {
            state = SessionState::default();
            seen_first_stdin = false;
            continue;
        }

        // `>` lines are stdin
        if line.starts_with("> ") {
            if seen_first_stdin {
                // Follow-up message — suppress the next turn separator
                state.suppress_next_separator = true;
            }
            seen_first_stdin = true;
            continue;
        }

        // `<` lines are stdout — parse and render
        if let Some(json) = line.strip_prefix("< ") {
            match parse_line(json) {
                Ok(Some(event)) => {
                    handle_inbound(&event, &mut state, &mut renderer, false);
                }
                Ok(None) => {}
                Err(e) => {
                    renderer.render_warning(&format!("Parse error: {e}"));
                }
            }
        }
    }

    drop(renderer);
    String::from_utf8(output).expect("Output should be valid UTF-8")
}

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

macro_rules! vcr_test {
    ($name:ident) => {
        #[test]
        fn $name() {
            let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
            let toml_path = base.join(concat!(stringify!($name), ".toml"));
            let vcr_path = base.join(concat!(stringify!($name), ".vcr"));

            let case: TestCase = toml::from_str(
                &std::fs::read_to_string(&toml_path).expect("Failed to read TOML file"),
            )
            .expect("Failed to parse TOML file");

            let vcr_content =
                std::fs::read_to_string(&vcr_path).expect("Failed to read VCR file");
            let vcr_lines: Vec<&str> = vcr_content.lines().collect();

            // Validate VCR header and stdin against TOML
            validate_vcr(&case, &vcr_lines);

            // Replay and snapshot
            let output = replay_stdout(&vcr_lines, &case.display);
            let clean = strip_ansi(&output);

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
vcr_test!(multi_tool);
vcr_test!(grep_glob);
vcr_test!(mcp_tool);
vcr_test!(error_handling);
vcr_test!(multi_turn);
vcr_test!(ralph_break);
vcr_test!(steering);
vcr_test!(subagent);
vcr_test!(write_single_line);
vcr_test!(edit_tool);

/// Test that --show-thinking streams thinking text inline.
/// Replays multi_tool.vcr (which contains thinking blocks) with show_thinking enabled.
#[test]
fn show_thinking() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let vcr_path = base.join("multi_tool.vcr");

    let vcr_content = std::fs::read_to_string(&vcr_path).expect("Failed to read VCR file");
    let vcr_lines: Vec<&str> = vcr_content.lines().collect();

    let display = DisplayConfig {
        show_thinking: true,
    };
    let output = replay_stdout(&vcr_lines, &display);
    let clean = strip_ansi(&output);

    insta::with_settings!({
        snapshot_path => "../tests/cases",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!("show_thinking", clean);
    });
}
