// Application settings – stored at ~/.config/fsn/settings.toml
//
// Contains user-level preferences: store URLs, UI language, etc.
// Loaded once at startup; saved when the user changes settings in the TUI.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::FsnError;

// ── AppSettings ───────────────────────────────────────────────────────────────

/// Global FSN application settings.
/// Persisted to `~/.config/fsn/settings.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Module stores to query when browsing or installing services.
    #[serde(default = "default_stores")]
    pub stores: Vec<StoreConfig>,

    /// Preferred UI language (BCP 47 code, e.g. "de", "fr").
    /// `None` = auto-detect from system locale.
    #[serde(default)]
    pub preferred_lang: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { stores: default_stores(), preferred_lang: None }
    }
}

fn default_stores() -> Vec<StoreConfig> {
    vec![StoreConfig {
        name:       "FSN Official".into(),
        url:        "https://raw.githubusercontent.com/FreeSynergy/Store/main".into(),
        git_url:    Some("https://github.com/FreeSynergy/Store.git".into()),
        local_path: None,
        enabled:    true,
    }]
}

// ── StoreConfig ───────────────────────────────────────────────────────────────

/// One configured module store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    /// Display name shown in the TUI Settings screen.
    pub name: String,

    /// Base URL of the store (used for index.toml and raw file downloads).
    /// The index is fetched from `{url}/Node/index.toml`.
    pub url: String,

    /// Git clone URL for syncing the full module tree locally.
    /// When absent, derived from `url` by stripping the raw.githubusercontent.com prefix.
    /// Example: "https://github.com/FreeSynergy/Store.git"
    #[serde(default)]
    pub git_url: Option<String>,

    /// Absolute local path to an already-checked-out Store directory.
    /// When set, `sync_modules` uses this path directly and skips git operations.
    /// Intended for development setups where the Store repo is already present.
    /// Example: "/home/kal/Server/Store"
    #[serde(default)]
    pub local_path: Option<String>,

    /// Whether this store is actively queried.
    /// Disabled stores are shown in Settings but not used.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

// ── Load / Save ───────────────────────────────────────────────────────────────

impl AppSettings {
    /// Load settings from `~/.config/fsn/settings.toml`.
    /// Returns `Default` when the file does not exist.
    pub fn load() -> Result<Self, FsnError> {
        let path = settings_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content).map_err(|e| FsnError::ConfigParse {
            path: path.display().to_string(),
            source: e,
        })
    }

    /// Persist settings to `~/.config/fsn/settings.toml`.
    pub fn save(&self) -> Result<(), FsnError> {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| FsnError::Template(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

/// Returns the platform-appropriate settings file path.
/// Uses `$HOME/.config/fsn/settings.toml` (XDG-compatible).
fn settings_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("fsn").join("settings.toml")
}

// ── Container Plugins directory ────────────────────────────────────────────────

/// Resolve the directory that holds container plugin definitions (formerly "modules/").
///
/// Priority (first match wins):
///   1. `FSN_PLUGINS_DIR` environment variable — explicit override.
///   2. First enabled store with a `local_path` set → `{local_path}/Node/`.
///   3. Legacy fallback: `{node_root}/modules/`.
///
/// Callers pass the FSN workspace root so the legacy path always resolves
/// even when no settings file or env var is present.
pub fn resolve_plugins_dir(node_root: &std::path::Path) -> PathBuf {
    if let Some(dir) = resolve_plugins_dir_no_fallback() {
        return dir;
    }
    // Legacy bundled modules directory.
    node_root.join("modules")
}

/// Resolve the plugins directory without requiring a `node_root` fallback.
///
/// Returns `None` when neither env var nor settings provide a path.
/// Used in contexts (TUI, web API) that do not have access to the Node workspace root.
///
/// Priority:
///   1. `FSN_PLUGINS_DIR` environment variable.
///   2. First enabled store with a `local_path` → `{local_path}/Node/`.
pub fn resolve_plugins_dir_no_fallback() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("FSN_PLUGINS_DIR") {
        return Some(PathBuf::from(dir));
    }
    if let Ok(settings) = AppSettings::load() {
        if let Some(store) = settings.stores.iter().find(|s| s.enabled && s.local_path.is_some()) {
            let base = PathBuf::from(store.local_path.as_deref().unwrap());
            return Some(base.join("Node").join("modules"));
        }
    }
    None
}
