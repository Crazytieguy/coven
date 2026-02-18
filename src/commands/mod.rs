pub mod gc;
pub mod init;
pub mod ralph;
pub mod run;
pub mod status;
pub mod worker;

use std::io::Write;
use std::path::Path;

use anyhow::Result;
use crossterm::terminal;

use crate::display::input::InputHandler;
use crate::display::renderer::Renderer;
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

/// Set up the display renderer and input handler with common configuration.
///
/// Caller is responsible for acquiring raw mode (via [`RawModeGuard`]) before
/// calling `renderer.render_hints()`.
pub(crate) fn setup_display<W: Write>(
    writer: W,
    term_width: Option<usize>,
    show_thinking: bool,
) -> (Renderer<W>, InputHandler) {
    let mut renderer = Renderer::with_writer(writer);
    if let Some(w) = term_width {
        renderer.set_width(w);
    }
    renderer.set_show_thinking(show_thinking);
    let input = InputHandler::new(2);
    (renderer, input)
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
