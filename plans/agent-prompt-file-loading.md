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

### New dependencies

- `serde_yaml` for YAML frontmatter parsing. It's the standard Rust YAML library and `serde` is already in the dependency tree.
- `handlebars` for template rendering. Mature crate (v6.x, actively maintained), already uses the `{{var}}` and `{{#if var}}` syntax shown in the file format above. Only dependency is `serde`. Supports custom helpers if we need them later.

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
- `AgentDef::render(&self, args: &HashMap<String, String>) -> Result<String>` — renders the prompt template with the given arguments using `handlebars`. Validates that all required args (per `frontmatter.args`) are present before rendering. Returns an error if a required arg is missing.

No registry struct — a `Vec<AgentDef>` is sufficient for now since there's no behavioral integration.

### Template rendering details

Use `handlebars::Handlebars` to render templates. The `render` method:

1. Checks that all args marked `required: true` in `frontmatter.args` are present in the provided map. Returns an error naming the missing arg(s) if not.
2. Creates a `Handlebars` instance with `strict_mode(false)` so that optional args that aren't provided simply render as empty rather than erroring.
3. Calls `handlebars.render_template(&self.prompt_template, &args)` to produce the final prompt string.

This gives us full Handlebars syntax: `{{var}}` for substitution, `{{#if var}}...{{/if}}` for conditionals, `{{#each items}}...{{/each}}` if needed later. The Handlebars instance is created per-render (not cached) since agent rendering is infrequent — we can optimize later if needed.

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
6. `render_with_all_args` — render a template with all args provided, verify substitution works.
7. `render_missing_required_arg` — render without a required arg returns an error.
8. `render_missing_optional_arg` — render without an optional arg succeeds, optional content omitted.
9. `render_conditional` — render a template using `{{#if var}}...{{/if}}`, verify conditional blocks work.

## Questions

### Should the agents directory path be configurable?

The issue specifies `.coven/agents/*.md`. Options:

- **Hardcoded relative path (recommended):** `load_agents` takes a base directory (the project root) and appends `.coven/agents/`. Simple and consistent.
- **Configurable:** Accept the full agents directory path. More flexible but over-engineered for now.

I'd go with hardcoded, using a constant like `AGENTS_DIR = ".coven/agents"`.

Answer: Constant is fine for now

## Review

