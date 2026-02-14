use std::collections::HashMap;
use std::fmt::Write as _;

use anyhow::{Context, Result};
use serde_yaml::Value;

use crate::agents::AgentDef;

/// Convert a YAML scalar value to a string representation.
/// Non-scalar values (sequences, mappings, tagged, null) return `None`.
fn yaml_scalar_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Null | Value::Sequence(_) | Value::Mapping(_) | Value::Tagged(_) => None,
    }
}

/// A transition declared by an agent via the `<next>` or `<wait-for-user>` tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    /// Hand off to the named agent with the given arguments.
    Next {
        agent: String,
        args: HashMap<String, String>,
    },
    /// No work available — sleep until new commits appear on main.
    Sleep,
    /// Agent is blocked on user input (e.g. permission prompt, clarification).
    WaitForUser { reason: String },
}

/// Parse a transition from an agent's text output.
///
/// Checks for `<wait-for-user>` first, then `<next>`. The rest of the
/// output is reasoning/status visible to the human and is ignored here.
pub fn parse_transition(text: &str) -> Result<Transition> {
    // Check for wait-for-user before next — agent is blocked on user input.
    if let Some(reason) = crate::protocol::parse::extract_tag_inner(text, "wait-for-user") {
        return Ok(Transition::WaitForUser {
            reason: reason.trim().to_string(),
        });
    }

    let yaml_str = extract_tag_content(text, "next")?;

    let value: Value =
        serde_yaml::from_str(&yaml_str).context("failed to parse transition YAML")?;
    let map = value
        .as_mapping()
        .context("transition content is not a YAML mapping")?;

    // Check for sleep
    if map
        .get(Value::String("sleep".into()))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(Transition::Sleep);
    }

    // Extract agent name
    let agent = map
        .get(Value::String("agent".into()))
        .and_then(|v| v.as_str())
        .context("transition YAML must contain 'agent' or 'sleep: true'")?
        .to_string();

    // Collect remaining fields as string arguments
    let args = map
        .iter()
        .filter_map(|(k, v)| {
            let key = k.as_str()?;
            if key == "agent" {
                return None;
            }
            let val = yaml_scalar_to_string(v)?;
            Some((key.to_string(), val))
        })
        .collect();

    Ok(Transition::Next { agent, args })
}

/// Format the transition protocol system prompt, including the agent catalog.
///
/// This text is injected into every agent session via `--append-system-prompt`.
/// It teaches the agent the `<next>` tag syntax and lists all available agents.
pub fn format_transition_system_prompt(agents: &[AgentDef]) -> String {
    let mut out = String::new();

    out.push_str("# Transition Protocol\n\n");
    out.push_str(
        "When you finish your work, output a <next> tag containing YAML to declare\n\
         what should happen next. This is how the orchestration system routes between\n\
         agents.\n\n",
    );

    out.push_str("## Hand off to another agent\n\n");
    out.push_str("<next>\nagent: <agent-name>\n<arg>: <value>\n</next>\n\n");

    out.push_str("## Sleep (no actionable work)\n\n");
    out.push_str("<next>\nsleep: true\n</next>\n\n");

    out.push_str("## Wait for user input\n\n");
    out.push_str(
        "If you need user input before you can proceed (e.g. a necessary command was denied,\n\
         or you need clarification on a requirement), output a `<wait-for-user>` tag:\n\n",
    );
    out.push_str("<wait-for-user>\nReason the user needs to act\n</wait-for-user>\n\n");
    out.push_str(
        "The orchestrator will show your reason, wait for the user to respond, and resume\n\
         your session with their input. After resuming, finish your work and output a `<next>` tag.\n\n",
    );

    out.push_str("## Available Agents\n\n");

    if agents.is_empty() {
        out.push_str("No agents configured.\n\n");
    } else {
        for agent in agents {
            let _ = writeln!(out, "### {}", agent.name);
            let _ = writeln!(out, "{}", agent.frontmatter.description);

            if agent.frontmatter.args.is_empty() {
                out.push_str("No arguments.\n");
            } else {
                out.push_str("Arguments:\n");
                for arg in &agent.frontmatter.args {
                    let req = if arg.required { " (required)" } else { "" };
                    let _ = writeln!(out, "- `{}`: {}{}", arg.name, arg.description, req);
                }
            }
            out.push('\n');
        }
    }

    out.push_str("## Examples\n\n");

    // Generate an example for each agent that has args
    let agents_with_args: Vec<_> = agents
        .iter()
        .filter(|a| !a.frontmatter.args.is_empty())
        .collect();
    for agent in &agents_with_args {
        let _ = write!(out, "<next>\nagent: {}\n", agent.name);
        for arg in &agent.frontmatter.args {
            let _ = writeln!(out, "{}: <{}>", arg.name, arg.description);
        }
        out.push_str("</next>\n\n");
    }

    // Example for an agent with no args (if any exist)
    if let Some(agent) = agents.iter().find(|a| a.frontmatter.args.is_empty()) {
        let _ = writeln!(out, "<next>\nagent: {}\n</next>\n", agent.name);
    }

    out.push_str("<next>\nsleep: true\n</next>\n");

    out
}

/// Build the corrective prompt for when a `<next>` tag is missing or malformed.
pub fn corrective_prompt(parse_err: &anyhow::Error) -> String {
    format!(
        "Your previous output could not be parsed: {parse_err}\n\n\
         Please output your decision inside a <next> tag containing YAML. \
         For example:\n\n\
         <next>\nagent: main\ntask: Example issue title\n</next>\n\n\
         Or to sleep:\n\n\
         <next>\nsleep: true\n</next>\n\n\
         Or if you need user input:\n\n\
         <wait-for-user>\nReason the user needs to act\n</wait-for-user>"
    )
}

/// Extract content between `<tag>` and `</tag>`.
fn extract_tag_content(text: &str, tag: &str) -> Result<String> {
    crate::protocol::parse::extract_tag_inner(text, tag)
        .map(|s| s.trim().to_string())
        .with_context(|| format!("no <{tag}>...</{tag}> found in agent output"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::agents::{AgentArg, AgentDef, AgentFrontmatter};

    #[test]
    fn parse_sleep() {
        let text = "No actionable work right now.\n\n<next>\nsleep: true\n</next>";
        let transition = parse_transition(text).unwrap();
        assert_eq!(transition, Transition::Sleep);
    }

    #[test]
    fn parse_agent_with_args() {
        let text = r"The scroll bug is highest priority.

<next>
agent: plan
issue: issues/fix-scroll-bug.md
</next>";

        let transition = parse_transition(text).unwrap();
        assert_eq!(
            transition,
            Transition::Next {
                agent: "plan".into(),
                args: HashMap::from([("issue".into(), "issues/fix-scroll-bug.md".into())]),
            }
        );
    }

    #[test]
    fn parse_agent_no_args() {
        let text = "Time for a routine audit.\n\n<next>\nagent: audit\n</next>";
        let transition = parse_transition(text).unwrap();
        assert_eq!(
            transition,
            Transition::Next {
                agent: "audit".into(),
                args: HashMap::new(),
            }
        );
    }

    #[test]
    fn parse_agent_multiple_args() {
        let text = "<next>\nagent: implement\nissue: issues/dark-mode.md\ncontext: depends on theme system\n</next>";
        let transition = parse_transition(text).unwrap();
        assert_eq!(
            transition,
            Transition::Next {
                agent: "implement".into(),
                args: HashMap::from([
                    ("issue".into(), "issues/dark-mode.md".into()),
                    ("context".into(), "depends on theme system".into()),
                ]),
            }
        );
    }

    #[test]
    fn parse_missing_tag() {
        let text = "I think we should work on the scroll bug.";
        let err = parse_transition(text).unwrap_err();
        assert!(
            err.to_string().contains("<next>"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_unclosed_tag() {
        let text = "<next>\nagent: plan\n";
        let err = parse_transition(text).unwrap_err();
        assert!(
            err.to_string().contains("<next>"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_invalid_yaml() {
        let text = "<next>\n: : : not yaml\n</next>";
        let err = parse_transition(text).unwrap_err();
        assert!(err.to_string().contains("parse"), "unexpected error: {err}");
    }

    #[test]
    fn parse_missing_agent_and_sleep() {
        let text = "<next>\npriority: high\n</next>";
        let err = parse_transition(text).unwrap_err();
        assert!(err.to_string().contains("agent"), "unexpected error: {err}");
    }

    #[test]
    fn parse_surrounding_text_ignored() {
        let text = "Lots of reasoning here.\n\nI considered the priorities and decided:\n\n<next>\nagent: plan\nissue: issues/foo.md\n</next>\n\nThis is the best choice because...";
        let transition = parse_transition(text).unwrap();
        assert_eq!(
            transition,
            Transition::Next {
                agent: "plan".into(),
                args: HashMap::from([("issue".into(), "issues/foo.md".into())]),
            }
        );
    }

    #[test]
    fn parse_non_string_args_converted() {
        let text = "<next>\nagent: implement\nissue: issues/fix-bug.md\npriority: 1\nverbose: true\n</next>";
        let transition = parse_transition(text).unwrap();
        assert_eq!(
            transition,
            Transition::Next {
                agent: "implement".into(),
                args: HashMap::from([
                    ("issue".into(), "issues/fix-bug.md".into()),
                    ("priority".into(), "1".into()),
                    ("verbose".into(), "true".into()),
                ]),
            }
        );
    }

    fn make_agent(name: &str, desc: &str, args: Vec<AgentArg>) -> AgentDef {
        AgentDef {
            name: name.into(),
            frontmatter: AgentFrontmatter {
                description: desc.into(),
                args,
                max_concurrency: None,
                claude_args: Vec::new(),
                title: None,
            },
            prompt_template: String::new(),
        }
    }

    #[test]
    fn system_prompt_lists_all_agents() {
        let agents = vec![
            make_agent("dispatch", "Chooses the next task", vec![]),
            make_agent("plan", "Plans work", vec![]),
        ];
        let prompt = format_transition_system_prompt(&agents);
        // All agents listed (including dispatch)
        assert!(prompt.contains("### dispatch"));
        assert!(prompt.contains("### plan"));
    }

    #[test]
    fn system_prompt_shows_args() {
        let agents = vec![make_agent(
            "plan",
            "Plans work",
            vec![AgentArg {
                name: "issue".into(),
                description: "The issue file".into(),
                required: true,
            }],
        )];
        let prompt = format_transition_system_prompt(&agents);
        assert!(prompt.contains("`issue`"));
        assert!(prompt.contains("(required)"));
        assert!(prompt.contains("The issue file"));
    }

    #[test]
    fn system_prompt_shows_no_args() {
        let agents = vec![make_agent("audit", "Reviews code quality", vec![])];
        let prompt = format_transition_system_prompt(&agents);
        assert!(prompt.contains("No arguments."));
    }

    #[test]
    fn system_prompt_examples() {
        let agents = vec![
            make_agent(
                "plan",
                "Plans work",
                vec![AgentArg {
                    name: "issue".into(),
                    description: "Path to issue".into(),
                    required: true,
                }],
            ),
            make_agent("audit", "Reviews code", vec![]),
        ];
        let prompt = format_transition_system_prompt(&agents);
        assert!(prompt.contains("agent: plan\nissue: <Path to issue>"));
        assert!(prompt.contains("agent: audit"));
        assert!(prompt.contains("sleep: true"));
    }

    #[test]
    fn system_prompt_empty_agents() {
        let prompt = format_transition_system_prompt(&[]);
        assert!(prompt.contains("No agents configured."));
    }

    #[test]
    fn system_prompt_contains_protocol() {
        let agents = vec![make_agent("plan", "Plans work", vec![])];
        let prompt = format_transition_system_prompt(&agents);
        assert!(prompt.contains("# Transition Protocol"));
        assert!(prompt.contains("<next>"));
        assert!(prompt.contains("</next>"));
    }

    #[test]
    fn parse_wait_for_user() {
        let text = "I need permission to run this command.\n\n<wait-for-user>\nThe `git push` command was denied. Please grant permission or push manually.\n</wait-for-user>";
        let transition = parse_transition(text).unwrap();
        assert_eq!(
            transition,
            Transition::WaitForUser {
                reason:
                    "The `git push` command was denied. Please grant permission or push manually."
                        .into(),
            }
        );
    }

    #[test]
    fn parse_wait_for_user_trims_whitespace() {
        let text = "<wait-for-user>  need clarification  </wait-for-user>";
        let transition = parse_transition(text).unwrap();
        assert_eq!(
            transition,
            Transition::WaitForUser {
                reason: "need clarification".into(),
            }
        );
    }

    #[test]
    fn parse_wait_for_user_takes_precedence_over_next() {
        // If both tags are present, wait-for-user wins (checked first).
        let text = "<wait-for-user>blocked</wait-for-user>\n<next>\nagent: plan\n</next>";
        let transition = parse_transition(text).unwrap();
        assert_eq!(
            transition,
            Transition::WaitForUser {
                reason: "blocked".into(),
            }
        );
    }

    #[test]
    fn system_prompt_documents_wait_for_user() {
        let agents = vec![make_agent("plan", "Plans work", vec![])];
        let prompt = format_transition_system_prompt(&agents);
        assert!(prompt.contains("<wait-for-user>"));
        assert!(prompt.contains("</wait-for-user>"));
    }
}
