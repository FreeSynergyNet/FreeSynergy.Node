use fsn_error::FsyError;
// Module + plugin registry – scans modules/ directory and loads all module
// class TOMLs and plugin TOMLs.
//
// Module layout:
//   Depth 3: modules/{type}/{name}/{name}.toml         → key "{type}/{name}"
//   Depth 4: modules/{type}/{parent}/{name}/{name}.toml → key "{type}/{parent}/{name}"
//
// Plugin layout:
//   Depth 5: modules/{type}/plugins/{plugin_type}/{name}.toml → key "{type}/{plugin_type}/{name}"

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::config::plugin::PluginConfig;
use crate::config::service::ServiceClass;


/// In-memory index of all available module classes and plugins.
///
/// Module key = "{type}/{name}" (e.g. "auth/kanidm", "git/forgejo")
/// Plugin key = "{type}/{plugin_type}/{name}" (e.g. "proxy/dns/hetzner")
#[derive(Debug, Default)]
pub struct ServiceRegistry {
    classes: HashMap<String, ServiceClass>,
    plugins: HashMap<String, PluginConfig>,
    /// Base path of the modules/ directory
    modules_dir: PathBuf,
}

impl ServiceRegistry {
    /// Scan a modules/ directory and load all class TOMLs.
    ///
    /// Two supported layouts:
    ///   Depth 3: `modules/{type}/{name}/{name}.toml`       → key = `{type}/{name}`
    ///   Depth 4: `modules/{type}/{parent}/{name}/{name}.toml` → key = `{type}/{parent}/{name}`
    ///
    /// Depth-4 enables sub-modules nested under a parent module
    /// (e.g. `proxy/zentinel/zentinel-control-plane`).
    pub fn load(modules_dir: &Path) -> Result<Self, FsyError> {
        let mut registry = Self {
            classes:     HashMap::new(),
            plugins:     HashMap::new(),
            modules_dir: modules_dir.to_path_buf(),
        };

        // ── Module class scan (depth 3–4) ─────────────────────────────────────
        for entry in WalkDir::new(modules_dir)
            .min_depth(3)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }

            // File name must match its parent directory (e.g. forgejo/forgejo.toml)
            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            let parent_name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or_default();

            if file_stem != parent_name {
                continue;
            }

            // Skip plugin directories (handled separately)
            if path.components().any(|c| c.as_os_str() == "plugins") {
                continue;
            }

            // Compute depth relative to modules_dir to pick the right key format
            let depth = path.components().count()
                .saturating_sub(modules_dir.components().count());

            let class_key = if depth == 3 {
                // modules/{type}/{name}/{name}.toml  →  {type}/{name}
                let type_name = path
                    .parent().and_then(|p| p.parent())
                    .and_then(|p| p.file_name()).and_then(|n| n.to_str())
                    .unwrap_or_default();
                format!("{type_name}/{file_stem}")
            } else {
                // modules/{type}/{parent}/{name}/{name}.toml  →  {type}/{parent}/{name}
                let parent_dir = path
                    .parent().and_then(|p| p.file_name()).and_then(|n| n.to_str())
                    .unwrap_or_default();
                let grandparent_dir = path
                    .parent().and_then(|p| p.parent()).and_then(|p| p.file_name()).and_then(|n| n.to_str())
                    .unwrap_or_default();
                let type_name = path
                    .parent().and_then(|p| p.parent()).and_then(|p| p.parent())
                    .and_then(|p| p.file_name()).and_then(|n| n.to_str())
                    .unwrap_or_default();
                format!("{type_name}/{grandparent_dir}/{parent_dir}")
            };

            match Self::load_class(path) {
                Ok(class) => { registry.classes.insert(class_key, class); }
                Err(e)    => { eprintln!("Warning: skipping {}: {}", path.display(), e); }
            }
        }

        // ── Plugin scan (depth 4): modules/{type}/plugins/{plugin_type}/{name}.toml ──
        for entry in WalkDir::new(modules_dir)
            .min_depth(4)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }

            // Must be under a "plugins" directory
            let is_plugin = path.components().any(|c| c.as_os_str() == "plugins");
            if !is_plugin { continue; }

            // Key: "{type}/{plugin_type}/{name}"  e.g. "proxy/dns/hetzner"
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let plugin_type = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or_default();
            let service_type = path
                .parent().and_then(|p| p.parent())  // plugins/
                .and_then(|p| p.parent())             // {type}/
                .and_then(|p| p.file_name()).and_then(|n| n.to_str())
                .unwrap_or_default();
            let plugin_key = format!("{service_type}/{plugin_type}/{name}");

            match Self::load_plugin(path) {
                Ok(plugin) => { registry.plugins.insert(plugin_key, plugin); }
                Err(e)     => { eprintln!("Warning: skipping plugin {}: {}", path.display(), e); }
            }
        }

        Ok(registry)
    }

    fn load_class(path: &Path) -> Result<ServiceClass, FsyError> {
        let content = std::fs::read_to_string(path).map_err(FsyError::Io)?;
        let p = path.display().to_string();
        toml::from_str(&content).map_err(|e| FsyError::Parse(format!("{p}: {e}")))
    }

    fn load_plugin(path: &Path) -> Result<PluginConfig, FsyError> {
        let content = std::fs::read_to_string(path).map_err(FsyError::Io)?;
        let p = path.display().to_string();
        toml::from_str(&content).map_err(|e| FsyError::Parse(format!("{p}: {e}")))
    }

    /// Look up a module class by its "{type}/{name}" key.
    pub fn get(&self, class_key: &str) -> Option<&ServiceClass> {
        self.classes.get(class_key)
    }

    /// Look up a plugin by service type, plugin type, and name.
    ///
    /// Example: `get_plugin("proxy", "dns", "hetzner")`
    pub fn get_plugin(&self, service_type: &str, plugin_type: &str, name: &str) -> Option<&PluginConfig> {
        let key = format!("{service_type}/{plugin_type}/{name}");
        self.plugins.get(&key)
    }

    /// All loaded module classes.
    pub fn all(&self) -> impl Iterator<Item = (&str, &ServiceClass)> {
        self.classes.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// All loaded plugins.
    pub fn all_plugins(&self) -> impl Iterator<Item = (&str, &PluginConfig)> {
        self.plugins.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn modules_dir(&self) -> &Path {
        &self.modules_dir
    }
}

