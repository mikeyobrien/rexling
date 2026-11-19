//! Settings kept in `$XDG_CONFIG_HOME/rexling/gtk.json`.
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub server: String,
    /// Every server that has been connected to, most recent first.
    pub servers: Vec<String>,
    pub font: String,
    pub font_size: f64,
    /// "system", "dark" or "light".
    pub appearance: String,
    pub theme_dark: String,
    pub theme_light: String,
    pub notifications: bool,
    /// Last session opened on each server, by endpoint.
    pub sessions: HashMap<String, String>,
    /// Last tab opened in each session.
    pub tabs: HashMap<String, String>,
    /// Last pane focused in each tab.
    pub panes: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: String::new(),
            servers: Vec::new(),
            font: "JetBrainsMono Nerd Font, monospace".into(),
            font_size: 11.0,
            appearance: "system".into(),
            theme_dark: crate::theme::DEFAULT_DARK.into(),
            theme_light: crate::theme::DEFAULT_LIGHT.into(),
            notifications: true,
            sessions: HashMap::new(),
            tabs: HashMap::new(),
            panes: HashMap::new(),
        }
    }
}

fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("rexling").join("gtk.json")
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(path()).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
    }
    pub fn save(&self) {
        let path = path();
        let result = (|| -> anyhow::Result<()> {
            std::fs::create_dir_all(path.parent().unwrap())?;
            let temporary = path.with_extension("json.tmp");
            std::fs::write(&temporary, serde_json::to_string_pretty(self)?)?;
            std::fs::rename(temporary, &path)?;
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("rexling: could not save {}: {error}", path.display());
        }
    }
}
