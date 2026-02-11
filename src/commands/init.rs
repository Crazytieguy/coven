use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::agents::AGENTS_DIR;
use crate::vcr::VcrContext;

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

To view all issues, list the `issues/` and `review/` directories. Read each file's YAML frontmatter to check its `state` and `priority` fields.

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
- If `review/` has several items, prefer implementing or sleeping over creating more plans. Don't overwhelm the human reviewer.
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

const WORKFLOW_DOC: &str = r"# Orchestration Workflow

This project uses [coven](https://github.com/yoavshapira/coven) for orchestrated development. Multiple workers run simultaneously, each picking up tasks from the issue queue.

## Issue Files

Issues are markdown files with YAML frontmatter in `issues/` or `review/`.

```yaml
---
priority: P1
state: new
---

# Fix scroll bug

Scroll position resets on window resize.
```

### Priorities

- `P0` — Critical, blocks other work
- `P1` — Normal priority (default)
- `P2` — Nice to have

### States

| State | Directory | Meaning |
|-------|-----------|---------|
| `new` | `issues/` | No plan yet — plan agent will pick it up |
| `review` | `review/` | Plan written, waiting for human review |
| `approved` | `issues/` | Human approved the plan, ready to implement |
| `changes-requested` | `issues/` | Human left feedback on the plan |
| `needs-replan` | `issues/` | Implementation failed, plan needs revision |

### Lifecycle

```
new → review              Plan agent writes plan, moves file to review/
review → approved         Human approves, moves file back to issues/
review → changes-requested  Human requests changes, moves file back to issues/
changes-requested → review  Plan agent revises, moves file to review/
approved → (deleted)      Implement agent succeeds, deletes the issue
approved → needs-replan   Implement agent fails, adds notes
needs-replan → review     Plan agent revises based on failure notes
```

## Creating Issues

Create a markdown file in `issues/` with the format above. Minimum fields: `state` and `priority` in frontmatter, plus a title and description. Commit the file.

**Skip path**: To skip planning and go straight to implementation, set `state: approved`.

## Reviewing Plans

Plans appear in `review/`. To review one:

1. Read the `## Plan` section and any `## Questions`
2. Answer questions inline (fill in below `**Answer:**` markers)
3. Update frontmatter: `state: approved` or `state: changes-requested`
4. Move the file from `review/` back to `issues/`
5. Commit

There's no time pressure — workers will wait or work on other issues.

## Directory Structure

```
issues/          Active issues (new, approved, changes-requested, needs-replan)
review/          Plans awaiting human review
.coven/
  agents/        Agent prompt templates
  workflow.md    This file
```
";

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

const COVEN_DIR: &str = ".coven";

/// Result of creating init files, used for VCR recording.
#[derive(Serialize, Deserialize)]
struct CreateFilesResult {
    created: Vec<String>,
    skipped: Vec<String>,
}

/// State of the CLAUDE.md file, used for VCR recording.
#[derive(Serialize, Deserialize)]
struct ClaudeMdState {
    needs_reference: bool,
    existing_content: Option<String>,
}

const WORKFLOW_REF: &str = "See @.coven/workflow.md for the issue-based development workflow.";

/// Create agent templates, workflow doc, and directory structure.
fn create_files(project_root: &Path) -> Result<CreateFilesResult> {
    let agents_dir = project_root.join(AGENTS_DIR);
    fs::create_dir_all(&agents_dir)
        .with_context(|| format!("failed to create {}", agents_dir.display()))?;

    let mut created = Vec::new();
    let mut skipped = Vec::new();

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

    let workflow_path = project_root.join(COVEN_DIR).join("workflow.md");
    if workflow_path.exists() {
        skipped.push(format!("{COVEN_DIR}/workflow.md"));
    } else {
        fs::write(&workflow_path, WORKFLOW_DOC)
            .with_context(|| format!("failed to write {}", workflow_path.display()))?;
        created.push(format!("{COVEN_DIR}/workflow.md"));
    }

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

    Ok(CreateFilesResult { created, skipped })
}

/// Check whether CLAUDE.md needs a workflow reference.
fn check_claude_md(project_root: &Path) -> Result<ClaudeMdState> {
    let claude_md_path = project_root.join("CLAUDE.md");
    let existing = if claude_md_path.exists() {
        Some(fs::read_to_string(&claude_md_path).context("failed to read CLAUDE.md")?)
    } else {
        None
    };
    let needs_reference = existing
        .as_ref()
        .is_none_or(|c| !c.contains(".coven/workflow.md"));
    Ok(ClaudeMdState {
        needs_reference,
        existing_content: existing,
    })
}

/// Write or update CLAUDE.md with the workflow reference.
fn update_claude_md(project_root: &Path, existing: Option<&String>) -> Result<()> {
    let claude_md_path = project_root.join("CLAUDE.md");
    if let Some(contents) = existing {
        let mut contents = contents.clone();
        if !contents.ends_with('\n') {
            contents.push('\n');
        }
        contents.push('\n');
        contents.push_str(WORKFLOW_REF);
        contents.push('\n');
        fs::write(&claude_md_path, contents).context("failed to update CLAUDE.md")?;
    } else {
        let contents = format!("{WORKFLOW_REF}\n");
        fs::write(&claude_md_path, contents).context("failed to create CLAUDE.md")?;
    }
    Ok(())
}

/// Initialize the project with default agent prompts and directory structure.
pub async fn init(
    vcr: &VcrContext,
    writer: &mut impl Write,
    stdin: &mut impl BufRead,
    working_dir: Option<PathBuf>,
) -> Result<()> {
    let project_root = super::resolve_working_dir(vcr, working_dir.as_deref()).await?;

    let result: CreateFilesResult = vcr
        .call(
            "init_create_files",
            project_root.clone(),
            async |root: &String| create_files(Path::new(root)),
        )
        .await?;

    if result.created.is_empty() {
        writeln!(writer, "Nothing to do — all files already exist.")?;
    } else {
        writeln!(writer, "Created:")?;
        for path in &result.created {
            writeln!(writer, "  {path}")?;
        }
    }
    if !result.skipped.is_empty() {
        writeln!(writer, "Skipped (already exist):")?;
        for path in &result.skipped {
            writeln!(writer, "  {path}")?;
        }
    }

    let claude_md: ClaudeMdState = vcr
        .call(
            "init_check_claude_md",
            project_root.clone(),
            async |root: &String| check_claude_md(Path::new(root)),
        )
        .await?;

    if claude_md.needs_reference {
        write!(
            writer,
            "\nAdd a reference to .coven/workflow.md in CLAUDE.md? [Y/n] "
        )?;
        writer.flush()?;

        let mut input_buf = String::new();
        stdin.read_line(&mut input_buf)?;
        let input = input_buf.trim();

        if input.is_empty() || input.eq_ignore_ascii_case("y") || input.eq_ignore_ascii_case("yes")
        {
            let existing = claude_md.existing_content.clone();
            vcr.call(
                "init_update_claude_md",
                project_root,
                async |root: &String| update_claude_md(Path::new(root), existing.as_ref()),
            )
            .await?;

            if claude_md.existing_content.is_some() {
                writeln!(writer, "Updated CLAUDE.md with workflow reference.")?;
            } else {
                writeln!(writer, "Created CLAUDE.md with workflow reference.")?;
            }
        } else {
            writeln!(
                writer,
                "\nTip: Add this to your CLAUDE.md so interactive sessions understand the workflow:"
            )?;
            writeln!(writer, "  {WORKFLOW_REF}")?;
        }
    }

    Ok(())
}
