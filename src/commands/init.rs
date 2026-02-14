use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::agents::AGENTS_DIR;
use crate::vcr::VcrContext;

const DISPATCH_PROMPT: &str = include_str!("../../.coven/agents/dispatch.md");
const MAIN_PROMPT: &str = include_str!("../../.coven/agents/main.md");
const LAND_SCRIPT: &str = include_str!("../../.coven/land.sh");
const SYSTEM_DOC: &str = include_str!("../../.coven/system.md");
const CONFIG_DOC: &str = include_str!("../../.coven/config.toml");

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
        path: "main.md",
        content: MAIN_PROMPT,
    },
];

const COVEN_DIR: &str = ".coven";

/// Result of creating init files, used for VCR recording.
#[derive(Serialize, Deserialize)]
struct CreateFilesResult {
    created: Vec<String>,
    skipped: Vec<String>,
}

const BRIEF_TEMPLATE: &str = "\
# Brief

Add tasks here — one per line or section. Workers pick them up automatically.

When workers have questions, they'll appear on `board.md` above the divider.
Write your answers here (just reference the issue by name) and commit.

You can also add general directives (\"prefer X over Y\", \"don't touch module Z\").
Workers read this file but never edit it.
";

const BOARD_TEMPLATE: &str = "\
# Board

---
";

/// Create agent templates, system doc, and project files.
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

    let system_path = project_root.join(COVEN_DIR).join("system.md");
    if system_path.exists() {
        skipped.push(format!("{COVEN_DIR}/system.md"));
    } else {
        fs::write(&system_path, SYSTEM_DOC)
            .with_context(|| format!("failed to write {}", system_path.display()))?;
        created.push(format!("{COVEN_DIR}/system.md"));
    }

    let config_path = project_root.join(COVEN_DIR).join("config.toml");
    if config_path.exists() {
        skipped.push(format!("{COVEN_DIR}/config.toml"));
    } else {
        fs::write(&config_path, CONFIG_DOC)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
        created.push(format!("{COVEN_DIR}/config.toml"));
    }

    let land_script_path = project_root.join(COVEN_DIR).join("land.sh");
    if land_script_path.exists() {
        skipped.push(format!("{COVEN_DIR}/land.sh"));
    } else {
        fs::write(&land_script_path, LAND_SCRIPT)
            .with_context(|| format!("failed to write {}", land_script_path.display()))?;
        created.push(format!("{COVEN_DIR}/land.sh"));
    }

    // Create brief.md and board.md at project root
    for (name, initial_content) in [("brief.md", BRIEF_TEMPLATE), ("board.md", BOARD_TEMPLATE)] {
        let path = project_root.join(name);
        if path.exists() {
            skipped.push(name.to_string());
        } else {
            fs::write(&path, initial_content).with_context(|| format!("failed to write {name}"))?;
            created.push(name.to_string());
        }
    }

    // Ensure scratch.md is gitignored
    let gitignore_path = project_root.join(".gitignore");
    let gitignore_content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).context("failed to read .gitignore")?
    } else {
        String::new()
    };
    if !gitignore_content.lines().any(|l| l.trim() == "scratch.md") {
        let mut new_content = gitignore_content;
        if !new_content.is_empty() && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str("scratch.md\n");
        fs::write(&gitignore_path, new_content).context("failed to update .gitignore")?;
        created.push(".gitignore (added scratch.md)".to_string());
    }

    Ok(CreateFilesResult { created, skipped })
}

/// Initialize the project with agent prompts and orchestration files.
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

    writeln!(writer)?;
    writeln!(
        writer,
        "Add tasks to brief.md and commit. Run `coven worker` to start."
    )?;
    writeln!(
        writer,
        "Workers will post questions to board.md — answer in brief.md."
    )?;

    let _ = stdin; // reserved for future interactive prompts

    Ok(())
}
