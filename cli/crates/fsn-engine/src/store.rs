// Store client – fetches indices and syncs module trees from configured stores.
//
// Each store is a Git repository with this structure:
//   Node/             ← module tree (plugin executables, templates, TOML)
//   Node/index.toml   ← module catalogue
//
// Two modes:
//   fetch_all()     – HTTP, fetches the TOML index only (for browsing)
//   sync_modules()  – git clone/pull, downloads the full module tree (for deploy)
//
// HTTP fetching is async (reqwest); git operations shell out to `git`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::info;

use fsn_core::{
    config::{AppSettings, ServiceRegistry},
    store::{StoreEntry, StoreIndex},
};

// ── StoreClient ───────────────────────────────────────────────────────────────

/// Manages store indices and local module availability.
pub struct StoreClient {
    /// User-configured stores (from AppSettings).
    settings: AppSettings,
    /// Local registry — used to check `is_local()`.
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

    /// Fetch the index from a store URL.
    ///
    /// Index URL: `{store_url}/Node/index.toml`.
    /// Returns an empty index on network error (caller shows "unavailable").
    pub async fn fetch_index(&self, store_url: &str) -> Result<StoreIndex> {
        let url = format!("{}/Node/index.toml", store_url.trim_end_matches('/'));
        let text = reqwest::get(&url)
            .await
            .with_context(|| format!("fetching store index from {url}"))?
            .text()
            .await
            .with_context(|| "reading store index response")?;
        toml::from_str(&text).with_context(|| format!("parsing store index from {url}"))
    }

    /// Fetch and merge all enabled store indices into a single list.
    ///
    /// Entries from earlier stores take precedence when IDs collide.
    /// Each `StoreEntry` is annotated with `is_local` at call time.
    pub async fn fetch_all(&self) -> Vec<StoreEntry> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for store in &self.settings.stores {
            if !store.enabled { continue }
            match self.fetch_index(&store.url).await {
                Ok(index) => {
                    for mut entry in index.modules {
                        if seen.insert(entry.id.clone()) {
                            entry.store_source = store.name.clone();
                            result.push(entry);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Store '{}' unavailable: {:#}", store.name, e);
                }
            }
        }
        result
    }

    /// Returns all entries for a given service type from the merged index.
    /// Used by the wizard to populate the service class dropdown.
    pub fn list_by_type<'a>(entries: &'a [StoreEntry], service_type: &str) -> Vec<&'a StoreEntry> {
        entries.iter()
            .filter(|e| e.primary_type_str() == service_type)
            .collect()
    }

    /// Load a bundled (offline) index from the local modules directory.
    ///
    /// Reads `{modules_dir}/../store/index.toml` — the index shipped with FSN.
    /// Falls back to an empty index when the file is absent.
    pub fn load_bundled(modules_dir: &Path) -> StoreIndex {
        let path = modules_dir.parent()
            .unwrap_or(modules_dir)
            .join("store")
            .join("index.toml");
        if !path.exists() { return StoreIndex::default(); }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Sync the module tree of the first enabled store to a local cache directory.
    ///
    /// Returns the path to the `Node/` subdirectory inside the synced store.
    /// This path is suitable for `DeployOpts::store_root`.
    ///
    /// Priority per store:
    ///   1. `local_path` set → use as-is, no git operations (dev mode)
    ///   2. `git_url` set → git clone/pull into `{cache_dir}/{store_name}/`
    ///   3. Derive git URL from `url` → same git clone/pull
    ///
    /// Cache directory: `~/.local/share/fsn/store/` (caller provides this).
    pub async fn sync_modules(&self, cache_dir: &Path) -> Result<PathBuf> {
        for store in &self.settings.stores {
            if !store.enabled { continue; }

            // ── Dev mode: local_path bypasses all git operations ──────────────
            if let Some(local) = &store.local_path {
                let node_dir = PathBuf::from(local).join("Node");
                if node_dir.exists() {
                    info!("Store '{}': using local path {}", store.name, node_dir.display());
                    return Ok(node_dir);
                }
                tracing::warn!(
                    "Store '{}': local_path set to '{}' but Node/ not found — skipping",
                    store.name, local
                );
                continue;
            }

            // ── Git sync ──────────────────────────────────────────────────────
            let git_url = store.git_url.as_deref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| raw_url_to_git(&store.url));

            let local_dir = cache_dir.join(name_to_slug(&store.name));

            sync_git_repo(&git_url, &local_dir).await
                .with_context(|| format!("syncing store '{}'", store.name))?;

            let node_dir = local_dir.join("Node");
            if node_dir.exists() {
                info!(
                    "Store '{}': synced → {}",
                    store.name, node_dir.display()
                );
                return Ok(node_dir);
            }
            tracing::warn!(
                "Store '{}': synced but no Node/ directory found in {}",
                store.name, local_dir.display()
            );
        }
        anyhow::bail!("no enabled store with a Node/ module tree could be synced")
    }
}

// ── Git helpers ───────────────────────────────────────────────────────────────

/// Clone (first run) or pull (subsequent runs) a git repository.
async fn sync_git_repo(git_url: &str, local_dir: &Path) -> Result<()> {
    if local_dir.join(".git").exists() {
        // Already cloned — fast-forward pull only (refuse merges)
        info!("git pull {}", local_dir.display());
        let status = tokio::process::Command::new("git")
            .args(["-C", &local_dir.to_string_lossy(), "pull", "--ff-only", "--quiet"])
            .status()
            .await
            .with_context(|| format!("git pull in {}", local_dir.display()))?;
        anyhow::ensure!(status.success(), "git pull failed in {}", local_dir.display());
    } else {
        // First run — shallow clone (depth 1 = only latest commit)
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
///
/// "https://raw.githubusercontent.com/Owner/Repo/branch"
/// → "https://github.com/Owner/Repo"
fn raw_url_to_git(raw_url: &str) -> String {
    let base = raw_url
        .trim_end_matches('/')
        .replace("://raw.githubusercontent.com/", "://github.com/");
    // Remove trailing /branch component
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
            raw_url_to_git("https://raw.githubusercontent.com/FreeSynergy/Store/main"),
            "https://github.com/FreeSynergy/Store"
        );
    }

    #[test]
    fn name_to_slug_works() {
        assert_eq!(name_to_slug("FSN Official"), "fsn-official");
    }
}
