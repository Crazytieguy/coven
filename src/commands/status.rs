use anyhow::Result;

use crate::worker_state::{self, StatusStyle};

/// Display the status of all active workers.
pub fn status() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let states = worker_state::read_all(&project_root)?;

    if states.is_empty() {
        println!("No active workers.");
        return Ok(());
    }

    println!("{} active worker(s):\n", states.len());
    print!(
        "{}",
        worker_state::format_workers(&states, StatusStyle::Cli)
    );

    Ok(())
}
