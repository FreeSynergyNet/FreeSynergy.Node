// Plugin configuration – helper configs per service type.
//
// Plugins are NOT modules. They provide type-level configuration
// (e.g. DNS providers, ACME providers) that all modules of the same type can use.
//
// File layout:  modules/{type}/plugins/{plugin_type}/{name}.toml
// Example key:  "proxy/dns/hetzner"
//
// Field order (mandatory): plugin → vars

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Root structure of a plugin config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub plugin: PluginMeta,

    /// Key-value pairs injected into the Jinja2 template context.
    /// Values may reference `{{ vault_* }}` secrets.
    #[serde(default)]
    pub vars: IndexMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    /// Machine name (e.g. "hetzner", "letsencrypt").
    pub name: String,

    /// Plugin category (e.g. "dns", "acme").
    #[serde(rename = "type")]
    pub plugin_type: String,

    pub description: Option<String>,
}
