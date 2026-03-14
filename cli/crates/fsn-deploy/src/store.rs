// Store client — fetches catalogs and syncs module trees.
//
// Architecture:
//   fsn_store::StoreClient  — generic I/O: Local or Http
//   StoreClient (this file) — FSN-specific: wraps fsn-store, adds git sync,
//                             multi-store merge, bundled offline fallback.
//
// Two modes:
//   fetch_all()     — catalog only (for browsing). Uses fsn_store::StoreClient.
//   sync_modules()  — full git clone/pull of the module tree (for deploy).
//
// StoreSource selection per configured store:
//   local_path set  → StoreSource::Local  (dev mode, no HTTP)
//   otherwise       → StoreSource::Http   (production)

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::info;

use fsn_core::{
    config::{AppSettings, ServiceRegistry},
    store::{StoreCatalog, StoreEntry},
};
use fsn_store::{StoreClient as SdkClient, StoreSource};

// ── StoreClient ───────────────────────────────────────────────────────────────

/// FSN store client — manages catalog fetching and local module availability.
pub struct StoreClient {
    settings: AppSettings,
    registry: ServiceRegistry,
}

impl StoreClient {
    pub fn new(settings: AppSettings, registry: ServiceRegistry) -> Self {
        Self { settings, registry }
    }

    /// Returns `true` when the module id is present in the local registry.
    pub fn is_local(&self, id: &str) -> bool {
        self.registry.get(id).is_some()
    }

    /// Fetch and merge all enabled store catalogs into a single list of packages.
    ///
    /// Per store:
    ///   - `local_path` set → `StoreSource::Local` (reads from disk, no HTTP)
    ///   - otherwise        → `StoreSource::Http`  (fetches from `store.url`)
    ///
    /// Entries from earlier stores take precedence when IDs collide.
    /// Each `StoreEntry` is annotated with `store_source` at call time.
    ///
    /// Returns `(entries, errors)`. Per-store failures are collected in `errors`
    /// so the caller can surface them while still returning partial results.
    pub async fn fetch_all(&self) -> (Vec<StoreEntry>, Vec<String>) {
        let mut seen   = std::collections::HashSet::new();
        let mut result = Vec::new();
        let mut errors = Vec::new();

        for store in &self.settings.stores {
            if !store.enabled { continue; }

            let source = if let Some(local) = &store.local_path {
                StoreSource::Local(PathBuf::from(local))
            } else {
                StoreSource::Http(store.url.clone())
            };

            let mut client = SdkClient::new(source);
            match client.fetch_catalog::<StoreEntry>("Node", false).await {
                Ok(catalog) => {
                    for mut entry in catalog.packages {
                        if seen.insert(entry.id.clone()) {
                            entry.store_source = store.name.clone();
                            result.push(entry);
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("Store '{}': {e}", store.name);
                    tracing::warn!("{}", msg);
                    errors.push(msg);
                }
            }
        }
        (result, errors)
    }

    /// Returns all entries for a given service type from the merged index.
    /// Used by the wizard to populate the service class dropdown.
    pub fn list_by_type<'a>(entries: &'a [StoreEntry], service_type: &str) -> Vec<&'a StoreEntry> {
        entries.iter()
            .filter(|e| e.primary_type_str() == service_type)
            .collect()
    }

    /// Load a bundled (offline) catalog from the local modules directory.
    ///
    /// Tries `{modules_dir}/../store/catalog.toml` (new format) then
    /// `{modules_dir}/../store/index.toml` (legacy). Falls back to empty catalog.
    pub fn load_bundled(modules_dir: &Path) -> StoreCatalog {
        let base = modules_dir.parent().unwrap_or(modules_dir).join("store");
        let catalog_path = base.join("catalog.toml");
        let index_path   = base.join("index.toml");

        let path = if catalog_path.exists() { &catalog_path }
                   else if index_path.exists() { &index_path }
                   else { return StoreCatalog::default(); };

        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Sync the module tree of the first enabled store to a local cache directory.
    ///
    /// Priority per store:
    ///   1. `local_path` set → use as-is (dev mode, no git)
    ///   2. `git_url` set    → git clone/pull into `{cache_dir}/{store_name}/`
    ///   3. Derive git URL from `url` → same git clone/pull
    pub async fn sync_modules(&self, cache_dir: &Path) -> Result<PathBuf> {
        for store in &self.settings.stores {
            if !store.enabled { continue; }

            if let Some(local) = &store.local_path {
                let node_dir = PathBuf::from(local).join("Node");
                if node_dir.exists() {
                    info!("Store '{}': using local path {}", store.name, node_dir.display());
                    return Ok(node_dir);
                }
                tracing::warn!(
                    "Store '{}': local_path '{}' has no Node/ dir — skipping",
                    store.name, local
                );
                continue;
            }

            let git_url = store.git_url.as_deref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| raw_url_to_git(&store.url));

            let local_dir = cache_dir.join(name_to_slug(&store.name));
            sync_git_repo(&git_url, &local_dir).await
                .with_context(|| format!("syncing store '{}'", store.name))?;

            let node_dir = local_dir.join("Node");
            if node_dir.exists() {
                info!("Store '{}': synced → {}", store.name, node_dir.display());
                return Ok(node_dir);
            }
            tracing::warn!(
                "Store '{}': synced but no Node/ directory in {}",
                store.name, local_dir.display()
            );
        }
        anyhow::bail!("no enabled store with a Node/ module tree could be synced")
    }
}

// ── Git helpers ───────────────────────────────────────────────────────────────

async fn sync_git_repo(git_url: &str, local_dir: &Path) -> Result<()> {
    if local_dir.join(".git").exists() {
        info!("git pull {}", local_dir.display());
        let status = tokio::process::Command::new("git")
            .args(["-C", &local_dir.to_string_lossy(), "pull", "--ff-only", "--quiet"])
            .status()
            .await
            .with_context(|| format!("git pull in {}", local_dir.display()))?;
        anyhow::ensure!(status.success(), "git pull failed in {}", local_dir.display());
    } else {
        if let Some(parent) = local_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        info!("git clone {} → {}", git_url, local_dir.display());
        let status = tokio::process::Command::new("git")
            .args(["clone", "--depth", "1", "--quiet", git_url, &local_dir.to_string_lossy()])
            .status()
            .await
            .with_context(|| format!("git clone {git_url}"))?;
        anyhow::ensure!(status.success(), "git clone failed for {git_url}");
    }
    Ok(())
}

/// Derive a git clone URL from a raw.githubusercontent.com URL.
fn raw_url_to_git(raw_url: &str) -> String {
    let base = raw_url
        .trim_end_matches('/')
        .replace("://raw.githubusercontent.com/", "://github.com/");
    match base.rfind('/') {
        Some(pos) => base[..pos].to_string(),
        None      => base,
    }
}

/// "FSN Official" → "fsn-official"
fn name_to_slug(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_url_to_git_works() {
        assert_eq!(
            raw_url_to_git("https://raw.githubusercontent.com/FreeSynergy/Node.Store/main"),
            "https://github.com/FreeSynergy/Node.Store"
        );
    }

    #[test]
    fn name_to_slug_works() {
        assert_eq!(name_to_slug("FSN Official"), "fsn-official");
    }
}
