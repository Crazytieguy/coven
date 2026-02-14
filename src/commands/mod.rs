pub mod gc;
pub mod init;
pub mod ralph;
pub mod run;
pub mod status;
pub mod worker;

use std::path::Path;

use anyhow::Result;
use crossterm::terminal;

use crate::vcr::VcrContext;

/// Guard that disables terminal raw mode on drop.
///
/// When `active` is true (live mode), raw mode was enabled on creation
/// and will be disabled on drop. When false (VCR replay), the guard is inert.
pub(crate) struct RawModeGuard {
    active: bool,
}

impl RawModeGuard {
    /// Enable raw mode if `live` is true. Returns an inert guard otherwise.
    pub fn acquire(live: bool) -> Result<Self> {
        if live {
            terminal::enable_raw_mode()?;
            Ok(Self { active: true })
        } else {
            Ok(Self { active: false })
        }
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.active {
            terminal::disable_raw_mode().ok();
        }
    }
}

/// Resolve the working directory through VCR. Uses the configured directory
/// if provided, otherwise falls back to `std::env::current_dir()`.
pub(crate) async fn resolve_working_dir(
    vcr: &VcrContext,
    working_dir: Option<&Path>,
) -> Result<String> {
    let configured_dir = working_dir.map(|d| d.display().to_string());
    vcr.call("current_dir", (), async |(): &()| {
        Ok(match &configured_dir {
            Some(d) => d.clone(),
            None => std::env::current_dir()?.display().to_string(),
        })
    })
    .await
}
