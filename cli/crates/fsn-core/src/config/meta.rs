// Common resource metadata — DRY base for all FSN managed objects.
//
// Design Pattern: Composition via #[serde(flatten)]
//   Every resource type (Project, Host, Service, Bot) embeds ResourceMeta
//   to share common fields: name, alias, description, version, tags.
//   Type-specific fields live on the outer struct.
//
// TOML compatibility: flatten means `name = "foo"` stays at the same level —
//   no nested `[meta]` table in the file.

use serde::{Deserialize, Serialize};

/// Common metadata shared by ALL FSN resources.
///
/// Embedded via `#[serde(flatten)]` into ProjectMeta, HostMeta,
/// ServiceInstanceMeta, BotMeta.  Provides the base fields that every
/// managed object needs for identification, display and filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMeta {
    /// Unique identifier within the resource's namespace.
    pub name: String,

    /// Display alias (optional).
    #[serde(default)]
    pub alias: Option<String>,

    /// One-line description of what this resource does.
    #[serde(default)]
    pub description: Option<String>,

    /// Semantic version string.
    #[serde(default = "default_version")]
    pub version: String,

    /// Free-form tags for filtering and grouping.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_version() -> String { "0.1.0".into() }

impl ResourceMeta {
    /// Returns the display name: alias if set, otherwise name.
    pub fn display_name(&self) -> &str {
        self.alias.as_deref().unwrap_or(&self.name)
    }
}
