// Store catalog data model — FSN-specific types that consume store-sdk.
//
// Architecture:
//   store-sdk  — generic Manifest trait, CatalogMeta, LocaleEntry, StoreClient
//   fsn-core   — StoreEntry (FSN package entry, implements Manifest), StoreCatalog
//   fsn-engine — StoreClient wraps store_sdk::StoreClient for FSN
//
// StoreCatalog is FSN's version of the catalog — it re-uses CatalogMeta and
// LocaleEntry from store-sdk but keeps FSN-specific fields on StoreEntry.
// The `alias = "modules"` on packages is FSN legacy backward-compat.

use serde::{Deserialize, Serialize};

use crate::config::service::types::{ServiceType, de_service_types};

// Re-export shared types so callers import from one place.
pub use store_sdk::{CatalogMeta, LocaleEntry};

// ── StoreCatalog ───────────────────────────────────────────────────────────────

/// FSN's top-level store catalog.
/// Deserializes `catalog.toml` fetched from `{store_url}/Node/catalog.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreCatalog {
    /// Catalog metadata — auto-generated header, informational only.
    #[serde(default)]
    pub catalog: CatalogMeta,

    /// All deployment packages listed in this catalog.
    /// Accepts both `[[packages]]` (catalog format) and `[[modules]]` (legacy).
    #[serde(default, alias = "modules")]
    pub packages: Vec<StoreEntry>,

    /// All available locale packs listed in this catalog.
    #[serde(default)]
    pub locales: Vec<LocaleEntry>,
}

// ── StoreEntry ────────────────────────────────────────────────────────────────

/// One package entry in the FSN catalog.
/// Describes a deployable service module (zentinel, kanidm, forgejo, …).
///
/// Implements `store_sdk::Manifest` so generic catalog infrastructure can
/// filter and look up entries without knowing FSN-specific fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreEntry {
    /// Unique identifier — matches the module class key in the local registry.
    /// Example: "iam/kanidm", "proxy/zentinel".
    pub id: String,

    /// Human-readable display name.
    /// Example: "Kanidm", "Zentinel".
    pub name: String,

    /// Dot-separated category following the Store category system.
    /// Example: "deploy.proxy", "deploy.iam", "deploy.git".
    #[serde(default)]
    pub category: String,

    /// Service type(s) — backward-compat alias for `category`.
    /// Accepts a single string (`service_type = "iam"`) or array.
    #[serde(
        alias = "service_type",
        rename = "service_types",
        deserialize_with = "de_service_types",
        default = "default_custom_types"
    )]
    pub service_types: Vec<ServiceType>,

    /// Version of the module definition (semver string).
    pub version: String,

    /// Short description of what the software does.
    pub description: String,

    /// Relative path to the SVG icon within the store repository.
    pub icon: Option<String>,

    /// SPDX license identifier. Example: "Apache-2.0", "MIT".
    #[serde(default)]
    pub license: Option<String>,

    /// Store-relative path to the package directory.
    #[serde(default)]
    pub path: Option<String>,

    /// Official website of the software this module deploys.
    pub website: Option<String>,

    /// Source code repository of the software.
    pub repository: Option<String>,

    /// Author / maintainer of the module definition.
    pub author: Option<String>,

    /// Searchable tags.
    #[serde(default)]
    pub tags: Vec<String>,

    /// ISO 8601 date string when this module was first published.
    #[serde(default)]
    pub created_at: Option<String>,

    /// ISO 8601 date string when this module was last updated.
    #[serde(default)]
    pub updated_at: Option<String>,

    /// Minimum FSN version required to deploy this module.
    #[serde(default)]
    pub min_fsn_version: Option<String>,

    /// Name of the store this entry was fetched from.
    /// Set by StoreClient when merging results from multiple stores.
    #[serde(default)]
    pub store_source: String,
}

fn default_custom_types() -> Vec<ServiceType> {
    vec![ServiceType::Custom]
}

// ── Manifest impl ─────────────────────────────────────────────────────────────

impl store_sdk::Manifest for StoreEntry {
    fn id(&self)       -> &str { &self.id }
    fn version(&self)  -> &str { &self.version }
    fn category(&self) -> &str { &self.category }
    fn name(&self)     -> &str { &self.name }
}

// ── StoreEntry methods ────────────────────────────────────────────────────────

impl StoreEntry {
    /// Returns the category-derived display label shown in the TUI service class dropdown.
    /// Format: "Kanidm (IAM)" or "Kanidm (IAM) ↓" when not installed locally.
    pub fn select_label(&self, is_local: bool) -> String {
        let type_label = if !self.category.is_empty() {
            self.category
                .split('.')
                .last()
                .unwrap_or(&self.category)
                .to_uppercase()
        } else {
            self.service_types.iter()
                .map(|t| t.label())
                .collect::<Vec<_>>()
                .join("/")
        };
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
    pub fn primary_type_str(&self) -> String {
        self.primary_type().to_string()
    }

    /// Returns the category suffix (e.g. "proxy" from "deploy.proxy").
    /// Falls back to primary_type_str for backward compat.
    pub fn category_type(&self) -> &str {
        if !self.category.is_empty() {
            self.category.split('.').last().unwrap_or(&self.category)
        } else {
            &self.category
        }
    }
}
