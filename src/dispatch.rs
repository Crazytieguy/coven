use std::collections::HashMap;
use std::fmt::Write as _;

use anyhow::{Context, Result};
use serde_yaml::Value;

use crate::agents::AgentDef;

/// A decision output by the dispatch agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchDecision {
    /// Run the named agent with the given arguments.
    RunAgent {
        agent: String,
        args: HashMap<String, String>,
    },
    /// No work available — sleep until new commits appear on main.
    Sleep,
}

/// Parse a dispatch decision from the agent's text output.
///
/// Looks for `<dispatch>...</dispatch>` containing YAML. The rest of the
/// output is reasoning/status visible to the human and is ignored here.
pub fn parse_decision(text: &str) -> Result<DispatchDecision> {
    let yaml_str = extract_tag_content(text, "dispatch")?;

    let value: Value = serde_yaml::from_str(&yaml_str).context("failed to parse dispatch YAML")?;
    let map = value
        .as_mapping()
        .context("dispatch content is not a YAML mapping")?;

    // Check for sleep
    if map
        .get(Value::String("sleep".into()))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(DispatchDecision::Sleep);
    }

    // Extract agent name
    let agent = map
        .get(Value::String("agent".into()))
        .and_then(|v| v.as_str())
        .context("dispatch YAML must contain 'agent' or 'sleep: true'")?
        .to_string();

    // Collect remaining fields as string arguments
    let args = map
        .iter()
        .filter_map(|(k, v)| {
            let key = k.as_str()?;
            if key == "agent" {
                return None;
            }
            let val = v.as_str()?;
            Some((key.to_string(), val.to_string()))
        })
        .collect();

    Ok(DispatchDecision::RunAgent { agent, args })
}

/// Format a catalog of available agents and the dispatch output syntax.
///
/// This text is injected into the dispatch prompt so the dispatch agent knows
/// what agents exist, what arguments they take, and how to format its output.
pub fn format_agent_catalog(agents: &[AgentDef]) -> String {
    let mut out = String::new();

    out.push_str("## Available Agents\n\n");

    let non_dispatch: Vec<_> = agents.iter().filter(|a| a.name != "dispatch").collect();

    if non_dispatch.is_empty() {
        out.push_str("No agents configured.\n\n");
    } else {
        for agent in &non_dispatch {
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

    out.push_str("## Dispatch Output Format\n\n");
    out.push_str("Output your decision inside a `<dispatch>` tag. The content is YAML.\n\n");

    // Generate an example for each non-dispatch agent
    for agent in &non_dispatch {
        let _ = write!(out, "```\n<dispatch>\nagent: {}\n", agent.name);
        for arg in &agent.frontmatter.args {
            let _ = writeln!(out, "{}: <{}>", arg.name, arg.description);
        }
        out.push_str("</dispatch>\n```\n\n");
    }

    out.push_str("To sleep (no actionable work available):\n\n");
    out.push_str("```\n<dispatch>\nsleep: true\n</dispatch>\n```\n");

    out
}

/// Extract content between `<tag>` and `</tag>`.
fn extract_tag_content(text: &str, tag: &str) -> Result<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text
        .find(&open)
        .with_context(|| format!("no <{tag}> tag found in dispatch output"))?;
    let after_open = start + open.len();
    let end = text[after_open..]
        .find(&close)
        .with_context(|| format!("no </{tag}> closing tag found in dispatch output"))?;
    Ok(text[after_open..after_open + end].trim().to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::agents::{AgentArg, AgentDef, AgentFrontmatter};

    #[test]
    fn parse_sleep() {
        let text = "No actionable work right now.\n\n<dispatch>\nsleep: true\n</dispatch>";
        let decision = parse_decision(text).unwrap();
        assert_eq!(decision, DispatchDecision::Sleep);
    }

    #[test]
    fn parse_agent_with_args() {
        let text = r"The scroll bug is highest priority.

<dispatch>
agent: plan
issue: issues/fix-scroll-bug.md
</dispatch>";

        let decision = parse_decision(text).unwrap();
        assert_eq!(
            decision,
            DispatchDecision::RunAgent {
                agent: "plan".into(),
                args: HashMap::from([("issue".into(), "issues/fix-scroll-bug.md".into())]),
            }
        );
    }

    #[test]
    fn parse_agent_no_args() {
        let text = "Time for a routine audit.\n\n<dispatch>\nagent: audit\n</dispatch>";
        let decision = parse_decision(text).unwrap();
        assert_eq!(
            decision,
            DispatchDecision::RunAgent {
                agent: "audit".into(),
                args: HashMap::new(),
            }
        );
    }

    #[test]
    fn parse_agent_multiple_args() {
        let text = "<dispatch>\nagent: implement\nissue: issues/dark-mode.md\ncontext: depends on theme system\n</dispatch>";
        let decision = parse_decision(text).unwrap();
        assert_eq!(
            decision,
            DispatchDecision::RunAgent {
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
        let err = parse_decision(text).unwrap_err();
        assert!(
            err.to_string().contains("no <dispatch> tag"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_unclosed_tag() {
        let text = "<dispatch>\nagent: plan\n";
        let err = parse_decision(text).unwrap_err();
        assert!(
            err.to_string().contains("no </dispatch> closing tag"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_invalid_yaml() {
        let text = "<dispatch>\n: : : not yaml\n</dispatch>";
        let err = parse_decision(text).unwrap_err();
        assert!(err.to_string().contains("parse"), "unexpected error: {err}");
    }

    #[test]
    fn parse_missing_agent_and_sleep() {
        let text = "<dispatch>\npriority: high\n</dispatch>";
        let err = parse_decision(text).unwrap_err();
        assert!(err.to_string().contains("agent"), "unexpected error: {err}");
    }

    #[test]
    fn parse_surrounding_text_ignored() {
        let text = "Lots of reasoning here.\n\nI considered the priorities and decided:\n\n<dispatch>\nagent: plan\nissue: issues/foo.md\n</dispatch>\n\nThis is the best choice because...";
        let decision = parse_decision(text).unwrap();
        assert_eq!(
            decision,
            DispatchDecision::RunAgent {
                agent: "plan".into(),
                args: HashMap::from([("issue".into(), "issues/foo.md".into())]),
            }
        );
    }

    fn make_agent(name: &str, desc: &str, args: Vec<AgentArg>) -> AgentDef {
        AgentDef {
            name: name.into(),
            frontmatter: AgentFrontmatter {
                description: desc.into(),
                args,
            },
            prompt_template: String::new(),
        }
    }

    #[test]
    fn catalog_excludes_dispatch() {
        let agents = vec![
            make_agent("dispatch", "The dispatch agent", vec![]),
            make_agent("plan", "Plans work", vec![]),
        ];
        let catalog = format_agent_catalog(&agents);
        assert!(!catalog.contains("### dispatch"));
        assert!(catalog.contains("### plan"));
    }

    #[test]
    fn catalog_shows_args() {
        let agents = vec![make_agent(
            "plan",
            "Plans work",
            vec![AgentArg {
                name: "issue".into(),
                description: "The issue file".into(),
                required: true,
            }],
        )];
        let catalog = format_agent_catalog(&agents);
        assert!(catalog.contains("`issue`"));
        assert!(catalog.contains("(required)"));
        assert!(catalog.contains("The issue file"));
    }

    #[test]
    fn catalog_shows_no_args() {
        let agents = vec![make_agent("audit", "Reviews code quality", vec![])];
        let catalog = format_agent_catalog(&agents);
        assert!(catalog.contains("No arguments."));
    }

    #[test]
    fn catalog_examples_per_agent() {
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
        let catalog = format_agent_catalog(&agents);
        // Should have dispatch examples for each agent
        assert!(catalog.contains("agent: plan\nissue: <Path to issue>"));
        assert!(catalog.contains("agent: audit"));
        // And the sleep example
        assert!(catalog.contains("sleep: true"));
    }

    #[test]
    fn catalog_empty_agents() {
        let catalog = format_agent_catalog(&[]);
        assert!(catalog.contains("No agents configured."));
    }
}
