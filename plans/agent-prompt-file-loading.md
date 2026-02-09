Issue: [P1] Agent prompt file loading — new module for loading agent definitions from `.coven/agents/*.md`, parsing YAML frontmatter (description, required arguments), providing a registry of available agent types. No behavioral integration yet.
Status: draft

## Approach

### File format

Agent definitions live in `.coven/agents/<name>.md`. Each file has YAML frontmatter delimited by `---` lines, followed by the agent's prompt body in markdown:

```markdown
---
description: "One-line description of this agent"
args:
  - name: issue
    description: "The issue to work on"
    required: true
  - name: context
    description: "Extra context"
---

You are an agent that works on {{issue}}.

{{#if context}}Additional context: {{context}}{{/if}}
```

The file stem (e.g. `reviewer` from `reviewer.md`) becomes the agent name.

### New dependency

Add `serde_yaml` for YAML frontmatter parsing. It's the standard Rust YAML library and `serde` is already in the dependency tree.

### New module: `src/agents.rs`

A single flat module (no subdirectory) since the scope is small.

**Types:**

```rust
/// A single argument definition for an agent.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentArg {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub required: bool,
}

/// YAML frontmatter parsed from an agent file.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentFrontmatter {
    pub description: String,
    #[serde(default)]
    pub args: Vec<AgentArg>,
}

/// A fully loaded agent definition.
#[derive(Debug, Clone)]
pub struct AgentDef {
    pub name: String,
    pub frontmatter: AgentFrontmatter,
    pub prompt_template: String,
}
```

**Public API:**

- `load_agent(path: &Path) -> Result<AgentDef>` — parse a single `.md` file. Splits on `---` delimiters, deserializes frontmatter with `serde_yaml`, captures the remainder as `prompt_template`.
- `load_agents(dir: &Path) -> Result<Vec<AgentDef>>` — glob `dir/*.md`, call `load_agent` on each, return all definitions sorted by name. Returns an empty vec (not an error) if the directory doesn't exist.

No registry struct — a `Vec<AgentDef>` is sufficient for now since there's no behavioral integration.

### Frontmatter parsing

Split file content on `---` lines. The content between the first and second `---` is YAML. Everything after the second `---` (trimmed) is the prompt template. If the file doesn't have valid frontmatter delimiters, return an error.

### Registration in `lib.rs`

Add `pub mod agents;` to `src/lib.rs`.

### Tests

Unit tests in `src/agents.rs`:

1. `parse_valid_agent` — parse a well-formed agent string with description, args, and prompt body.
2. `parse_no_args` — agent with description but no args field.
3. `parse_missing_frontmatter` — file without `---` delimiters returns an error.
4. `load_agents_missing_dir` — `load_agents` on a nonexistent directory returns empty vec.
5. `load_agents_from_dir` — write temp files, load them, verify names and count.

## Questions

### Should argument substitution be part of this module?

The issue says "no behavioral integration yet." We could either:

- **Template-only (recommended):** Store `prompt_template` as a raw string. Argument substitution is a future concern when we actually use agents. This keeps the module focused on loading/parsing.
- **Include substitution:** Add a `render(args: &HashMap<String, String>) -> Result<String>` method now. Would require a templating library or manual `{{arg}}` replacement.

I'd go with template-only — keep it simple and defer substitution to the integration issue.

Answer:

### Should the agents directory path be configurable?

The issue specifies `.coven/agents/*.md`. Options:

- **Hardcoded relative path (recommended):** `load_agents` takes a base directory (the project root) and appends `.coven/agents/`. Simple and consistent.
- **Configurable:** Accept the full agents directory path. More flexible but over-engineered for now.

I'd go with hardcoded, using a constant like `AGENTS_DIR = ".coven/agents"`.

Answer:

## Review

