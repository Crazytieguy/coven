use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// Relative path from project root to the agents directory.
pub const AGENTS_DIR: &str = ".coven/agents";

/// A single argument definition for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentArg {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub required: bool,
}

/// YAML frontmatter parsed from an agent file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFrontmatter {
    pub description: String,
    #[serde(default)]
    pub args: Vec<AgentArg>,
    pub max_concurrency: Option<u32>,
    /// Extra arguments to pass to the `claude` CLI when running this agent.
    #[serde(default)]
    pub claude_args: Vec<String>,
    /// Optional Handlebars template for the terminal title.
    /// Rendered with the same args map used for the prompt.
    pub title: Option<String>,
}

/// A fully loaded agent definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDef {
    pub name: String,
    pub frontmatter: AgentFrontmatter,
    pub prompt_template: String,
}

/// Parse frontmatter and prompt body from file contents.
///
/// Expects the file to start with `---`, followed by YAML, then another `---`,
/// then the prompt template.
fn parse_agent_file(contents: &str) -> Result<(AgentFrontmatter, String)> {
    let trimmed = contents.trim_start();
    if !trimmed.starts_with("---") {
        bail!("agent file must start with `---` frontmatter delimiter");
    }
    // Skip the first `---` line
    let after_first = &trimmed[3..];
    let after_first = after_first.strip_prefix('\n').unwrap_or(after_first);

    let Some(end_idx) = after_first.find("\n---") else {
        bail!("agent file missing closing `---` frontmatter delimiter");
    };
    let yaml_str = &after_first[..end_idx];
    let rest = &after_first[end_idx + 4..]; // skip "\n---"
    let prompt_template = rest.strip_prefix('\n').unwrap_or(rest).trim().to_string();

    let frontmatter: AgentFrontmatter =
        serde_yaml::from_str(yaml_str).context("failed to parse agent frontmatter YAML")?;

    Ok((frontmatter, prompt_template))
}

/// Load a single agent definition from a `.md` file.
pub fn load_agent(path: &Path) -> Result<AgentDef> {
    let name = path
        .file_stem()
        .context("agent file has no stem")?
        .to_str()
        .context("agent file name is not valid UTF-8")?
        .to_string();

    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read agent file: {}", path.display()))?;

    let (frontmatter, prompt_template) =
        parse_agent_file(&contents).with_context(|| format!("in file: {}", path.display()))?;

    Ok(AgentDef {
        name,
        frontmatter,
        prompt_template,
    })
}

/// Load all agent definitions from a directory.
///
/// Globs `dir/*.md`, loads each file, and returns definitions sorted by name.
/// Returns an empty vec if the directory doesn't exist.
pub fn load_agents(dir: &Path) -> Result<Vec<AgentDef>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut agents = Vec::new();
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read agents directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let agent = load_agent(&path)?;
            agents.push(agent);
        }
    }

    agents.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(agents)
}

impl AgentDef {
    /// Render the title template with the given arguments, if one is configured.
    ///
    /// Returns `None` if no title template is set.
    pub fn render_title(&self, args: &HashMap<String, String>) -> Result<Option<String>> {
        let Some(template) = &self.frontmatter.title else {
            return Ok(None);
        };

        let mut hbs = handlebars::Handlebars::new();
        hbs.set_strict_mode(false);
        hbs.register_escape_fn(handlebars::no_escape);
        let rendered = hbs
            .render_template(template, args)
            .context("failed to render title template")?;
        Ok(Some(rendered))
    }

    /// Render the prompt template with the given arguments.
    ///
    /// Validates that all required args are present, then uses Handlebars
    /// to render the template.
    pub fn render(&self, args: &HashMap<String, String>) -> Result<String> {
        // Check for missing required args
        let missing: Vec<&str> = self
            .frontmatter
            .args
            .iter()
            .filter(|a| a.required && !args.contains_key(&a.name))
            .map(|a| a.name.as_str())
            .collect();

        if !missing.is_empty() {
            bail!("missing required argument(s): {}", missing.join(", "));
        }

        let mut hbs = handlebars::Handlebars::new();
        hbs.set_strict_mode(false);
        hbs.register_escape_fn(handlebars::no_escape);
        hbs.render_template(&self.prompt_template, args)
            .context("failed to render agent template")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const VALID_AGENT: &str = r#"---
description: "A code review agent"
args:
  - name: issue
    description: "The issue to review"
    required: true
  - name: context
    description: "Extra context"
---

You are a code reviewer working on {{issue}}.

{{#if context}}Additional context: {{context}}{{/if}}"#;

    #[test]
    fn parse_valid_agent() {
        let (fm, body) = parse_agent_file(VALID_AGENT).unwrap();
        assert_eq!(fm.description, "A code review agent");
        assert_eq!(fm.args.len(), 2);
        assert_eq!(fm.args[0].name, "issue");
        assert!(fm.args[0].required);
        assert_eq!(fm.args[1].name, "context");
        assert!(!fm.args[1].required);
        assert!(body.contains("You are a code reviewer"));
    }

    #[test]
    fn parse_no_args() {
        let input = "---\ndescription: \"Simple agent\"\n---\n\nDo the thing.";
        let (fm, body) = parse_agent_file(input).unwrap();
        assert_eq!(fm.description, "Simple agent");
        assert!(fm.args.is_empty());
        assert_eq!(body, "Do the thing.");
    }

    #[test]
    fn parse_missing_frontmatter() {
        let input = "No frontmatter here.";
        let err = parse_agent_file(input).unwrap_err();
        assert!(
            err.to_string().contains("frontmatter delimiter"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn load_agents_missing_dir() {
        let result = load_agents(Path::new("/nonexistent/path/agents")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn load_agents_from_dir() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("beta.md"),
            "---\ndescription: \"Beta\"\n---\n\nBeta prompt.",
        )
        .unwrap();
        fs::write(
            dir.path().join("alpha.md"),
            "---\ndescription: \"Alpha\"\n---\n\nAlpha prompt.",
        )
        .unwrap();
        // Non-md file should be ignored
        fs::write(dir.path().join("ignore.txt"), "not an agent").unwrap();

        let agents = load_agents(dir.path()).unwrap();
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].name, "alpha");
        assert_eq!(agents[1].name, "beta");
    }

    #[test]
    fn render_with_all_args() {
        let (fm, body) = parse_agent_file(VALID_AGENT).unwrap();
        let agent = AgentDef {
            name: "test".into(),
            frontmatter: fm,
            prompt_template: body,
        };
        let mut args = HashMap::new();
        args.insert("issue".into(), "fix the bug".into());
        args.insert("context".into(), "it's urgent".into());
        let rendered = agent.render(&args).unwrap();
        assert!(rendered.contains("fix the bug"));
        assert!(rendered.contains("it's urgent"));
    }

    #[test]
    fn render_missing_required_arg() {
        let (fm, body) = parse_agent_file(VALID_AGENT).unwrap();
        let agent = AgentDef {
            name: "test".into(),
            frontmatter: fm,
            prompt_template: body,
        };
        let args = HashMap::new();
        let err = agent.render(&args).unwrap_err();
        assert!(err.to_string().contains("issue"), "unexpected error: {err}");
    }

    #[test]
    fn render_missing_optional_arg() {
        let (fm, body) = parse_agent_file(VALID_AGENT).unwrap();
        let agent = AgentDef {
            name: "test".into(),
            frontmatter: fm,
            prompt_template: body,
        };
        let mut args = HashMap::new();
        args.insert("issue".into(), "fix the bug".into());
        // "context" is optional, omit it
        let rendered = agent.render(&args).unwrap();
        assert!(rendered.contains("fix the bug"));
        assert!(!rendered.contains("Additional context"));
    }

    #[test]
    fn parse_max_concurrency() {
        let input = "---\ndescription: \"Limited agent\"\nmax_concurrency: 2\n---\n\nDo the thing.";
        let (fm, _body) = parse_agent_file(input).unwrap();
        assert_eq!(fm.max_concurrency, Some(2));
    }

    #[test]
    fn parse_no_max_concurrency() {
        let input = "---\ndescription: \"Unlimited agent\"\n---\n\nDo the thing.";
        let (fm, _body) = parse_agent_file(input).unwrap();
        assert_eq!(fm.max_concurrency, None);
    }

    #[test]
    fn parse_claude_args() {
        let input = r#"---
description: "Agent with CLI args"
claude_args:
  - "--allowedTools"
  - "Bash(git add:*),Bash(git commit:*)"
---

Do the thing."#;
        let (fm, _body) = parse_agent_file(input).unwrap();
        assert_eq!(fm.claude_args.len(), 2);
        assert_eq!(fm.claude_args[0], "--allowedTools");
        assert_eq!(fm.claude_args[1], "Bash(git add:*),Bash(git commit:*)");
    }

    #[test]
    fn parse_claude_args_defaults_empty() {
        let input = "---\ndescription: \"No claude args\"\n---\n\nDo the thing.";
        let (fm, _body) = parse_agent_file(input).unwrap();
        assert!(fm.claude_args.is_empty());
    }

    #[test]
    fn render_conditional() {
        let input = "---\ndescription: \"Conditional\"\nargs:\n  - name: verbose\n    description: \"Enable verbose\"\n---\n\nBase prompt.\n\n{{#if verbose}}Verbose mode enabled: {{verbose}}{{/if}}";
        let (fm, body) = parse_agent_file(input).unwrap();
        let agent = AgentDef {
            name: "test".into(),
            frontmatter: fm,
            prompt_template: body,
        };

        // With the arg
        let mut args = HashMap::new();
        args.insert("verbose".into(), "yes".into());
        let rendered = agent.render(&args).unwrap();
        assert!(rendered.contains("Verbose mode enabled: yes"));

        // Without the arg
        let args_empty = HashMap::new();
        let rendered = agent.render(&args_empty).unwrap();
        assert!(!rendered.contains("Verbose mode enabled"));
    }

    #[test]
    fn render_title_with_template() {
        let input = "---\ndescription: \"Worker\"\ntitle: \"{{task}}\"\nargs:\n  - name: task\n    description: \"The task\"\n    required: true\n---\n\nDo {{task}}.";
        let (fm, body) = parse_agent_file(input).unwrap();
        assert_eq!(fm.title.as_deref(), Some("{{task}}"));
        let agent = AgentDef {
            name: "main".into(),
            frontmatter: fm,
            prompt_template: body,
        };
        let mut args = HashMap::new();
        args.insert("task".into(), "Fix the bug".into());
        let title = agent.render_title(&args).unwrap();
        assert_eq!(title.as_deref(), Some("Fix the bug"));
    }

    #[test]
    fn render_title_none_when_absent() {
        let input = "---\ndescription: \"Simple\"\n---\n\nDo the thing.";
        let (fm, body) = parse_agent_file(input).unwrap();
        assert!(fm.title.is_none());
        let agent = AgentDef {
            name: "test".into(),
            frontmatter: fm,
            prompt_template: body,
        };
        let title = agent.render_title(&HashMap::new()).unwrap();
        assert!(title.is_none());
    }
}
