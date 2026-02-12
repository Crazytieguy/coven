use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

const CONFIG_PATH: &str = ".coven/config.toml";

/// Project-level coven configuration from `.coven/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Which agent runs first when a worker starts or wakes from sleep.
    #[serde(default = "default_entry_agent")]
    pub entry_agent: String,
}

fn default_entry_agent() -> String {
    "dispatch".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            entry_agent: default_entry_agent(),
        }
    }
}

/// Load configuration from `.coven/config.toml` under `worktree_path`.
///
/// Falls back to defaults if the file is missing.
pub fn load(worktree_path: &Path) -> Result<Config> {
    let path = worktree_path.join(CONFIG_PATH);
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = std::fs::read_to_string(&path)?;
    let config: Config = toml::from_str(&contents)?;
    Ok(config)
}
