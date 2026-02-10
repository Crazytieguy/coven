pub mod gc;
pub mod init;
pub mod ralph;
pub mod run;
pub mod session_loop;
pub mod status;
pub mod worker;

use anyhow::Result;
use crossterm::terminal;

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
