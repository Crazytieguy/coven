use std::fs;

use anyhow::{Context, Result};

use coven::agents::AGENTS_DIR;

const DISPATCH_PROMPT: &str = r#"---
description: "Chooses the next task for a worker"
args:
  - name: agent_catalog
    description: "Available agents and dispatch syntax"
    required: true
  - name: worker_status
    description: "What other workers are currently doing"
    required: true
---

You are the dispatch agent. Decide what this worker should do next.

## Issue System

Issues are markdown files with YAML frontmatter:
- `issues/` — active issues (states: `new`, `approved`, `changes-requested`, `needs-replan`)
- `review/` — plans awaiting human review (state: `review`)

Start by listing both directories to see what's available, then read relevant issue files.

### States and Routing

| State | Meaning | Route to |
|-------|---------|----------|
| `new` | No plan yet | `plan` agent |
| `changes-requested` | Human left feedback on plan | `plan` agent |
| `needs-replan` | Implementation failed | `plan` agent |
| `approved` | Plan approved, ready to build | `implement` agent |
| `review` | Awaiting human review | Do not assign |

### Priorities

- Issue frontmatter has a `priority` field: `P0` > `P1` > `P2`.
- Prefer implementing approved issues over planning new ones at the same priority.
- Don't assign work another worker is already doing.
- If nothing is actionable (everything in review, or no issues), sleep.
- Consider codebase locality — avoid conflicts with other workers.

## Current Worker Status

{{worker_status}}

{{agent_catalog}}

## Instructions

Briefly explain your reasoning (visible to the human), then output your decision."#;

const PLAN_PROMPT: &str = r#"---
description: "Writes a plan for an issue"
args:
  - name: issue
    description: "Path to the issue file"
    required: true
---

You are the plan agent. Write an implementation plan for the issue at `{{issue}}`.

## Steps

1. Read the issue file
2. Explore the codebase enough to write a concrete plan
3. Write a `## Plan` section in the issue file
4. If anything is ambiguous, add a `## Questions` section with specific questions for the human
5. Update the frontmatter: set `state: review`
6. Move the file from `issues/` to `review/`
7. Commit with a message describing what you planned

## Revising a Plan

If the state is `changes-requested` or `needs-replan`, the issue already has a plan and feedback. Read the existing plan and any comments, revise accordingly, then move to `review/` with `state: review`.

## Splitting

If the issue is too large for one implementation session, rewrite the original to cover the first piece and create new issue files in `issues/` for the rest (state: `new`, same priority).

## Guidelines

- Plans should be specific enough to implement without re-deriving decisions
- Surface ambiguity as questions rather than guessing
- Keep plans focused — one atomic change per issue"#;

const IMPLEMENT_PROMPT: &str = r#"---
description: "Implements code changes for a planned issue"
args:
  - name: issue
    description: "Path to the issue file"
    required: true
---

You are the implement agent. Implement the plan in the issue at `{{issue}}`.

## Steps

1. Read the issue file — it contains the problem description and plan
2. Implement the plan
3. Run tests and fix any failures your changes introduce
4. Run the linter and fix any warnings

## On Success

- Delete the issue file
- Commit all changes with a descriptive message

## On Failure

If you can't complete the implementation (plan is wrong, unexpected blocker, change is too large):

- Update the issue frontmatter: set `state: needs-replan`
- Add a `## Implementation Notes` section explaining what went wrong
- Commit the updated issue file (don't commit broken code)

## Noticing Other Issues

If you spot unrelated bugs or tech debt, create new issue files in `issues/` (state: `new`, priority: `P2`). Don't fix them now."#;

const AUDIT_PROMPT: &str = r#"---
description: "Reviews codebase for quality issues and test gaps"
---

You are the audit agent. Perform a routine review of the codebase.

## Steps

1. Look for code quality issues, test gaps, potential bugs, and technical debt
2. Check existing issues first to avoid duplicates
3. For each finding, create an issue file in `issues/` with:
   - A descriptive filename (kebab-case)
   - YAML frontmatter with `priority` (P0 for bugs, P1 for quality, P2 for nice-to-haves) and `state: new`
   - A clear description of the issue
4. Commit all new issue files

## Focus Areas

- Untested code paths
- Error handling gaps
- Code that doesn't match project conventions
- Performance or security concerns

## Guidelines

- Don't fix issues yourself — just document them
- Be specific: reference file paths, function names, line numbers
- Prioritize actionable findings over stylistic preferences"#;

struct TemplateFile {
    path: &'static str,
    content: &'static str,
}

const AGENT_TEMPLATES: &[TemplateFile] = &[
    TemplateFile {
        path: "dispatch.md",
        content: DISPATCH_PROMPT,
    },
    TemplateFile {
        path: "plan.md",
        content: PLAN_PROMPT,
    },
    TemplateFile {
        path: "implement.md",
        content: IMPLEMENT_PROMPT,
    },
    TemplateFile {
        path: "audit.md",
        content: AUDIT_PROMPT,
    },
];

/// Initialize the project with default agent prompts and directory structure.
pub fn init() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let agents_dir = project_root.join(AGENTS_DIR);

    // Create .coven/agents/
    fs::create_dir_all(&agents_dir)
        .with_context(|| format!("failed to create {}", agents_dir.display()))?;

    let mut created = Vec::new();
    let mut skipped = Vec::new();

    // Write agent prompt templates
    for template in AGENT_TEMPLATES {
        let path = agents_dir.join(template.path);
        if path.exists() {
            skipped.push(format!("{AGENTS_DIR}/{}", template.path));
        } else {
            fs::write(&path, template.content)
                .with_context(|| format!("failed to write {}", path.display()))?;
            created.push(format!("{AGENTS_DIR}/{}", template.path));
        }
    }

    // Create issues/ and review/ directories with .gitkeep
    for dir_name in ["issues", "review"] {
        let dir = project_root.join(dir_name);
        if dir.exists() {
            skipped.push(format!("{dir_name}/"));
        } else {
            fs::create_dir_all(&dir)
                .with_context(|| format!("failed to create {}", dir.display()))?;
            fs::write(dir.join(".gitkeep"), "")
                .with_context(|| format!("failed to write {dir_name}/.gitkeep"))?;
            created.push(format!("{dir_name}/"));
        }
    }

    // Print summary
    if created.is_empty() {
        println!("Nothing to do — all files already exist.");
    } else {
        println!("Created:");
        for path in &created {
            println!("  {path}");
        }
    }

    if !skipped.is_empty() {
        println!("Skipped (already exist):");
        for path in &skipped {
            println!("  {path}");
        }
    }

    Ok(())
}
