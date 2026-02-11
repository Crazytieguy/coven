use std::io::Write;
use std::path::Path;

use anyhow::Result;

use crate::vcr::VcrContext;
use crate::worker_state::{self, StatusStyle};

/// Display the status of all active workers.
pub async fn status(
    vcr: &VcrContext,
    working_dir: Option<&Path>,
    writer: &mut impl Write,
) -> Result<()> {
    let project_root = super::resolve_working_dir(vcr, working_dir).await?;

    let states = vcr
        .call(
            "worker_state::read_all",
            project_root,
            async |p: &String| worker_state::read_all(Path::new(p)),
        )
        .await?;

    if states.is_empty() {
        writeln!(writer, "No active workers.")?;
        return Ok(());
    }

    writeln!(writer, "{} active worker(s):\n", states.len())?;
    write!(
        writer,
        "{}",
        worker_state::format_workers(&states, StatusStyle::Cli)
    )?;

    Ok(())
}
