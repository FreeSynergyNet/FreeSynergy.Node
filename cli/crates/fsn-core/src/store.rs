// Store index data model.
//
// The store is a module registry (like apt/npm) distributed as a
// `store/index.toml` file at the root of any store repository.
//
// Field order in index.toml (mandatory): id → name → service_type → version
//   → description → icon → website → repository → author → tags

use serde::{Deserialize, Serialize};

use crate::config::service::types::{ServiceType, de_service_types};

// ── StoreIndex ────────────────────────────────────────────────────────────────

/// The top-level store manifest.
/// Fetched from `{store_url}/store/index.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreIndex {
    /// All modules listed in this store.
    #[serde(default)]
    pub modules: Vec<StoreEntry>,
}

// ── StoreEntry ────────────────────────────────────────────────────────────────

/// One module entry in the store index.
/// Describes a deployable service (kanidm, forgejo, zentinel, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreEntry {
    /// Unique identifier — matches the module class key in the local registry.
    /// Example: "iam/kanidm", "proxy/zentinel".
    pub id: String,

    /// Human-readable display name.
    /// Example: "Kanidm", "Zentinel".
    pub name: String,

    /// Service type(s) — accepts either a single string (legacy `service_type`)
    /// or an array (`service_types`). Deserialized via `de_service_types`.
    /// Example: `service_type = "iam"` or `service_types = ["proxy", "webhoster_simple"]`.
    #[serde(
        alias = "service_type",
        rename = "service_types",
        deserialize_with = "de_service_types",
        default = "default_custom_types"
    )]
    pub service_types: Vec<ServiceType>,

    /// Version of the module definition.
    pub version: String,

    /// Short description of what the software does.
    pub description: String,

    /// Relative path to the SVG icon within the store repository.
    /// Used by the web UI — terminal falls back to the module name.
    pub icon: Option<String>,

    /// Official website of the software this module deploys.
    /// Example: "https://kanidm.com" — link to project documentation.
    pub website: Option<String>,

    /// Source code repository of the software (not the module definition).
    /// Example: "https://github.com/kanidm/kanidm".
    pub repository: Option<String>,

    /// Author / maintainer of the module definition.
    pub author: Option<String>,

    /// Searchable tags for the store browser.
    #[serde(default)]
    pub tags: Vec<String>,

    /// ISO 8601 date string when this module was first published.
    #[serde(default)]
    pub created_at: Option<String>,

    /// ISO 8601 date string when this module was last updated.
    #[serde(default)]
    pub updated_at: Option<String>,

    /// License identifier (SPDX), e.g. "Apache-2.0", "MIT".
    #[serde(default)]
    pub license: Option<String>,

    /// Minimum FSN version required to deploy this module.
    /// Example: "0.2.0".
    #[serde(default)]
    pub min_fsn_version: Option<String>,

    /// Name of the store this entry was fetched from.
    /// Set by StoreClient when merging results from multiple stores.
    /// Empty string means source is unknown (e.g. bundled index).
    #[serde(default)]
    pub store_source: String,
}

fn default_custom_types() -> Vec<ServiceType> {
    vec![ServiceType::Custom]
}

impl StoreEntry {
    /// Returns the formatted label shown in the TUI service class dropdown.
    /// Format: "Kanidm (IAM)" or "Kanidm (IAM) ↓" when not installed locally.
    pub fn select_label(&self, is_local: bool) -> String {
        let type_label = self.service_types.iter()
            .map(|t| t.label())
            .collect::<Vec<_>>()
            .join("/");
        if is_local {
            format!("{} ({})", self.name, type_label)
        } else {
            format!("{} ({}) ↓", self.name, type_label)
        }
    }

    /// Returns the primary (first) service type of this entry.
    pub fn primary_type(&self) -> &ServiceType {
        self.service_types.first().unwrap_or(&ServiceType::Custom)
    }

    /// Returns the primary service type as a lowercase string.
    /// Used for backward-compat comparisons with legacy string-based filters.
    pub fn primary_type_str(&self) -> String {
        self.primary_type().to_string()
    }
}
