use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiPreferences {
    pub sidebar_open: bool,
    pub show_thinking: bool,
    pub show_timestamps: bool,
    pub animations_enabled: bool,
    pub theme: Option<String>,
}

impl Default for TuiPreferences {
    fn default() -> Self {
        Self {
            sidebar_open: false,
            show_thinking: true,
            show_timestamps: false,
            animations_enabled: true,
            theme: None,
        }
    }
}

impl TuiPreferences {
    pub fn load() -> Self {
        match Self::path() {
            Ok(path) if path.exists() => fs::read_to_string(&path)
                .ok()
                .and_then(|content| serde_yaml::from_str(&content).ok())
                .unwrap_or_default(),
            _ => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    fn path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Failed to resolve config directory")?
            .join(env!("CARGO_CRATE_NAME"));
        Ok(config_dir.join("tui.yaml"))
    }
}
