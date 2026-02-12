use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::agents::AGENTS_DIR;
use crate::vcr::VcrContext;

const DISPATCH_PROMPT: &str = include_str!("../../.coven/agents/dispatch.md");
const PLAN_PROMPT: &str = include_str!("../../.coven/agents/plan.md");
const IMPLEMENT_PROMPT: &str = include_str!("../../.coven/agents/implement.md");
const LAND_PROMPT: &str = include_str!("../../.coven/agents/land.md");
const LAND_SCRIPT: &str = include_str!("../../.coven/land.sh");
const WORKFLOW_DOC: &str = include_str!("../../.coven/workflow.md");

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
        path: "land.md",
        content: LAND_PROMPT,
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

    let land_script_path = project_root.join(COVEN_DIR).join("land.sh");
    if land_script_path.exists() {
        skipped.push(format!("{COVEN_DIR}/land.sh"));
    } else {
        fs::write(&land_script_path, LAND_SCRIPT)
            .with_context(|| format!("failed to write {}", land_script_path.display()))?;
        created.push(format!("{COVEN_DIR}/land.sh"));
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
