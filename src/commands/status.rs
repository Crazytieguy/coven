use anyhow::Result;

use crate::worker_state;

/// Display the status of all active workers.
pub fn status() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let states = worker_state::read_all(&project_root)?;

    if states.is_empty() {
        println!("No active workers.");
        return Ok(());
    }

    println!("{} active worker(s):\n", states.len());
    for state in &states {
        match &state.agent {
            Some(agent) => {
                let mut args_parts: Vec<_> =
                    state.args.iter().map(|(k, v)| format!("{k}={v}")).collect();
                args_parts.sort();
                if args_parts.is_empty() {
                    println!("  {} (PID {}) — {agent}", state.branch, state.pid);
                } else {
                    let args_str = args_parts.join(", ");
                    println!(
                        "  {} (PID {}) — {agent} ({args_str})",
                        state.branch, state.pid
                    );
                }
            }
            None => {
                println!("  {} (PID {}) — idle", state.branch, state.pid);
            }
        }
    }

    Ok(())
}
