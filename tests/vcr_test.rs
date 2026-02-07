use std::io::Write;
use std::path::Path;

use coven::display::renderer::Renderer;
use coven::protocol::parse::parse_line;
use coven::protocol::types::{InboundEvent, SystemEvent};
use coven::session::state::{SessionState, SessionStatus};

/// Replay a VCR fixture through the renderer, capturing output to a string.
fn replay_vcr(vcr_path: &Path) -> String {
    let vcr_content = std::fs::read_to_string(vcr_path).expect("Failed to read VCR file");
    let lines: Vec<&str> = vcr_content.lines().collect();

    // First line is header — skip it
    assert!(
        lines[0].contains("\"_vcr\""),
        "First line should be VCR header"
    );

    let mut output = Vec::new();
    let mut renderer = Renderer::with_writer(&mut output);
    let mut state = SessionState::default();

    for line in &lines[1..] {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // `>` lines are stdin (sent to claude) — we skip them during replay
        if line.starts_with("> ") {
            continue;
        }

        // `<` lines are stdout (from claude) — parse and feed to renderer
        if let Some(json) = line.strip_prefix("< ") {
            match parse_line(json) {
                Ok(Some(event)) => {
                    handle_event(&event, &mut state, &mut renderer);
                }
                Ok(None) => {}
                Err(e) => {
                    renderer.render_warning(&format!("Parse error: {e}"));
                }
            }
        }
    }

    // Flush
    drop(renderer);
    String::from_utf8(output).expect("Output should be valid UTF-8")
}

fn handle_event<W: Write>(
    event: &InboundEvent,
    state: &mut SessionState,
    renderer: &mut Renderer<W>,
) {
    match event {
        InboundEvent::System(SystemEvent::Init(init)) => {
            state.session_id = Some(init.session_id.clone());
            state.model = Some(init.model.clone());
            state.status = SessionStatus::Running;
            renderer.render_session_header(&init.session_id, &init.model);
        }
        InboundEvent::System(SystemEvent::Other) => {}
        InboundEvent::StreamEvent(se) => {
            renderer.handle_stream_event(se);
        }
        InboundEvent::Assistant(_) => {}
        InboundEvent::User(u) => {
            if let Some(ref result) = u.tool_use_result {
                renderer.render_tool_result(result);
            }
        }
        InboundEvent::Result(result) => {
            state.total_cost_usd = result.total_cost_usd;
            state.num_turns = result.num_turns;
            state.duration_ms = result.duration_ms;
            state.status = SessionStatus::WaitingForInput;
            renderer.render_result(
                &result.subtype,
                result.total_cost_usd,
                result.duration_ms,
                result.num_turns,
            );
        }
    }
}

/// Strip ANSI escape codes from output for readable snapshots.
fn strip_ansi(s: &str) -> String {
    // Simple regex-free ANSI stripping
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until we hit a letter (end of escape sequence)
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

#[test]
fn test_simple_qa() {
    let vcr_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases/simple_qa.vcr");
    let output = replay_vcr(&vcr_path);
    let clean = strip_ansi(&output);

    insta::with_settings!({
        snapshot_path => "../tests/cases",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!("simple_qa", clean);
    });
}

#[test]
fn test_tool_use() {
    let vcr_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases/tool_use.vcr");
    let output = replay_vcr(&vcr_path);
    let clean = strip_ansi(&output);

    insta::with_settings!({
        snapshot_path => "../tests/cases",
        prepend_module_to_snapshot => false,
    }, {
        insta::assert_snapshot!("tool_use", clean);
    });
}
