// fsn-tui — Terminal UI for FreeSynergy.Node.
//
// Entry point: `run(root)` — called by `fsn tui`.
// Detects whether a project exists → Welcome screen or Dashboard.

/// Build timestamp (set by build.rs, e.g. "2026-03-07 14:22").
pub const BUILD_TIME: &str = env!("FSN_BUILD_TIME");
/// Short git commit hash (set by build.rs, e.g. "a1b2c3d").
pub const GIT_HASH:   &str = env!("FSN_GIT_HASH");

pub mod actions;
pub mod app;
pub mod bot_form;
pub mod click_map;
pub mod deploy_thread;
pub mod events;
pub mod events_dashboard;
pub mod form_queue;
pub mod handles;
pub mod host_form;
pub mod i18n;
pub mod mouse;
pub mod project_form;
pub mod resource_form;
pub mod schema_form;
pub mod service_form;
pub mod settings_form;
pub mod submit;
pub mod sysinfo;
pub mod task_queue;
pub mod ui;

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;

use app::AppState;
use handles::{HostHandle, ProjectHandle, RunState, ServiceHandle};
use sysinfo::SysInfo;

// ── Background store fetcher ──────────────────────────────────────────────────

/// Fetch the store index from all enabled stores in a background thread.
///
/// Sends the merged entry list back via channel once the HTTP requests
/// complete. The main loop picks it up and updates `state.store_entries`.
/// Called at startup so the wizard always has fresh module options,
/// even when the bundled offline index is absent or stale.
pub fn spawn_store_fetcher(
    settings: fsn_core::config::AppSettings,
) -> mpsc::Receiver<Vec<fsn_core::store::StoreEntry>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let rt      = tokio::runtime::Runtime::new().expect("tokio runtime");
        let entries = rt.block_on(async move {
            let registry = fsn_core::config::ServiceRegistry::default();
            let client   = fsn_engine::store::StoreClient::new(settings, registry);
            client.fetch_all().await
        });
        let _ = tx.send(entries);
    });
    rx
}

// ── Store language index ──────────────────────────────────────────────────────

/// A language entry from the Store's `Node/i18n/index.toml`.
/// Fetched at startup by `spawn_lang_index_fetcher` and stored in `AppState::store_langs`.
#[derive(Debug, Clone)]
pub struct StoreLangEntry {
    pub code:         String,
    pub name:         String,
    pub api_version:  u32,
    pub completeness: u8,
}

/// Fetch the language index from the first enabled store in a background thread.
///
/// Tries local_path first (offline), then HTTP. Sends the entry list via channel.
/// The main loop picks it up and stores it in `state.store_langs`.
pub fn spawn_lang_index_fetcher(
    settings: fsn_core::config::AppSettings,
) -> mpsc::Receiver<Result<Vec<StoreLangEntry>, String>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = fetch_lang_index(&settings).map_err(|e| e.to_string());
        let _ = tx.send(result);
    });
    rx
}

fn fetch_lang_index(settings: &fsn_core::config::AppSettings) -> anyhow::Result<Vec<StoreLangEntry>> {
    #[derive(serde::Deserialize)]
    struct Entry { code: String, name: String, api_version: u32, completeness: u8 }
    #[derive(serde::Deserialize)]
    struct Index { languages: Vec<Entry> }

    let content = if let Some(store) = settings.stores.iter().find(|s| s.enabled && s.local_path.is_some()) {
        let path = std::path::PathBuf::from(store.local_path.as_deref().unwrap())
            .join("Node").join("i18n").join("index.toml");
        std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("local read: {e}"))?
    } else if let Some(store) = settings.stores.iter().find(|s| s.enabled) {
        let url = format!("{}/Node/i18n/index.toml", store.url.trim_end_matches('/'));
        let rt  = tokio::runtime::Runtime::new()?;
        rt.block_on(async move {
            let resp = reqwest::get(&url).await?.error_for_status()?;
            resp.text().await.map_err(anyhow::Error::from)
        })?
    } else {
        anyhow::bail!("no enabled store configured");
    };

    let index: Index = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("parse error: {e}"))?;
    Ok(index.languages.into_iter().map(|e| StoreLangEntry {
        code:         e.code,
        name:         e.name,
        api_version:  e.api_version,
        completeness: e.completeness,
    }).collect())
}

// ── Background language downloader ───────────────────────────────────────────

/// Download a language TOML file from the first enabled store and save it to
/// `~/.local/share/fsn/i18n/{code}.toml`.
///
/// Returns `Ok(code)` on success, `Err(message)` on failure.
/// The caller should set `state.lang_download_rx` to the returned receiver and
/// call `state.reload_langs()` when the result arrives (handled in event_loop.rs).
pub fn spawn_lang_downloader(
    code:     &str,
    settings: fsn_core::config::AppSettings,
) -> mpsc::Receiver<Result<String, String>> {
    let code = code.to_string();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = download_lang(&code, &settings);
        let _ = tx.send(result.map(|_| code).map_err(|e| e.to_string()));
    });
    rx
}

fn download_lang(code: &str, settings: &fsn_core::config::AppSettings) -> anyhow::Result<()> {
    // Determine source: local_path first (fast, no HTTP required), then HTTP.
    let content = if let Some(store) = settings.stores.iter().find(|s| s.enabled && s.local_path.is_some()) {
        let path = std::path::PathBuf::from(store.local_path.as_deref().unwrap())
            .join("Node").join("i18n").join(format!("{code}.toml"));
        std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("local read: {e}"))?
    } else if let Some(store) = settings.stores.iter().find(|s| s.enabled) {
        let url = format!("{}/Node/i18n/{code}.toml", store.url.trim_end_matches('/'));
        let rt  = tokio::runtime::Runtime::new()?;
        rt.block_on(async move {
            let resp = reqwest::get(&url).await?.error_for_status()?;
            resp.text().await.map_err(anyhow::Error::from)
        })?
    } else {
        anyhow::bail!("no enabled store configured");
    };

    // Validate that the TOML is parseable before saving.
    crate::i18n::DynamicLang::load(&content)
        .map_err(|e| anyhow::anyhow!("invalid language file: {e}"))?;

    // Save to the user's i18n directory.
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let dir  = std::path::PathBuf::from(home).join(".local").join("share").join("fsn").join("i18n");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("{code}.toml")), content)?;
    Ok(())
}

// ── Background reconciler ─────────────────────────────────────────────────────

/// Spawn a background thread that periodically queries Podman and sends
/// container name → RunState maps back to the main loop.
pub fn spawn_reconciler(interval: Duration) -> mpsc::Receiver<HashMap<String, RunState>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || loop {
        let statuses = podman_container_statuses();
        if tx.send(statuses).is_err() { break; }  // main thread dropped receiver → exit
        std::thread::sleep(interval);
    });
    rx
}

/// Query `podman ps -a` and return a map of container name → RunState.
fn podman_container_statuses() -> HashMap<String, RunState> {
    let out = std::process::Command::new("podman")
        .args(["ps", "-a", "--format", "{{.Names}}|{{.Status}}"])
        .output();

    let Ok(output) = out else { return HashMap::new() };
    let text = String::from_utf8_lossy(&output.stdout);

    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '|');
            let name   = parts.next()?.trim().to_string();
            let status = parts.next().unwrap_or("").trim();
            if name.is_empty() { return None; }
            let run_state = if status.starts_with("Up") {
                RunState::Running
            } else if status.starts_with("Exited") {
                RunState::Stopped
            } else {
                RunState::Missing
            };
            Some((name, run_state))
        })
        .collect()
}

/// Start the TUI. Blocks until the user quits.
/// Terminal setup (raw mode, alternate screen, mouse capture) is managed by rat-salsa.
pub fn run(root: &Path) -> Result<()> {
    let sysinfo = SysInfo::collect();
    let (projects, project_errors) = load_projects(root);
    let mut state = AppState::new(sysinfo, projects);

    // Surface load errors as startup notifications (broken/invalid TOML files).
    for msg in project_errors {
        state.push_notif(app::NotifKind::Info, msg);
    }

    // Load the bundled store index (offline — no HTTP required at startup).
    let store_index = fsn_engine::store::StoreClient::load_bundled(&root.join("modules"));
    state.store_entries = store_index.modules;

    // Load hosts for the first selected project.
    if let Some(proj) = state.projects.first() {
        let project_dir = root.join("projects").join(&proj.slug);
        let (hosts, host_errors) = load_hosts(&project_dir);
        state.hosts = hosts;
        for msg in host_errors {
            state.push_notif(app::NotifKind::Info, msg);
        }
        state.rebuild_sidebar();
    }

    // Build initial service list from desired state + Podman query.
    state.apply_podman_status(podman_container_statuses());

    // Navigate straight to Dashboard if a project.toml exists.
    if project_toml_exists(root) {
        state.screen = app::Screen::Dashboard;
    }

    // Fetch fresh store index from HTTP in the background.
    let store_fetcher_rx = if state.settings.stores.iter().any(|s| s.enabled) {
        Some(spawn_store_fetcher(state.settings.clone()))
    } else {
        None
    };
    state.store_rx = store_fetcher_rx;

    // Fetch language index from Store in the background.
    // Always spawned — even if no store is configured, the fetcher returns a
    // descriptive error that the Tick handler surfaces as a notification.
    state.store_langs_rx = Some(spawn_lang_index_fetcher(state.settings.clone()));

    // Start background reconciler (polls Podman every 5 seconds).
    // The receiver lives in AppState so the rat-salsa Tick handler can drain it.
    state.reconcile_rx = Some(spawn_reconciler(Duration::from_secs(5)));

    app::run_salsa(root.to_path_buf(), &mut state)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Load all projects from `root/projects/` using `ProjectConfig::load()`.
///
/// Returns `(valid_projects, load_errors)`. Broken or invalid TOML files are
/// skipped and their error messages collected in the second return value so the
/// caller can surface them as UI notifications.
pub fn load_projects(root: &Path) -> (Vec<ProjectHandle>, Vec<String>) {
    let projects_dir = root.join("projects");
    if !projects_dir.exists() { return (vec![], vec![]); }

    let mut projects = Vec::new();
    let mut errors   = Vec::new();
    let Ok(entries) = std::fs::read_dir(&projects_dir) else { return (projects, errors); };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let Ok(inner) = std::fs::read_dir(&path) else { continue; };
        for f in inner.flatten() {
            let fp = f.path();
            let is_project_toml = fp.extension().and_then(|e| e.to_str()) == Some("toml")
                && fp.file_stem().and_then(|s| s.to_str())
                    .map(|s| s.ends_with(".project"))
                    .unwrap_or(false);
            if !is_project_toml { continue; }

            let stem = fp.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let slug = stem.strip_suffix(".project").unwrap_or(stem).to_string();

            match fsn_core::config::project::ProjectConfig::load(&fp) {
                Ok(mut config) => {
                    // Merge standalone .service.toml files — source of truth for env + all fields.
                    merge_service_instances(&mut config, &path);
                    projects.push(ProjectHandle { slug, toml_path: fp, config });
                }
                Err(e) => {
                    errors.push(format!("Broken project file '{}': {e}", fp.display()));
                }
            }
        }
    }
    (projects, errors)
}

/// Merge standalone `.service.toml` files from `{project_dir}/services/` into the
/// project config's `load.services` map.
///
/// Standalone files are the single source of truth for per-service data (env vars,
/// subdomain, port, tags, …).  Services present only in standalone files (not yet
/// referenced from the project TOML) are inserted automatically so they always
/// appear in the sidebar and can be edited.
fn merge_service_instances(
    config:      &mut fsn_core::config::project::ProjectConfig,
    project_dir: &Path,
) {
    use fsn_core::config::project::{ServiceEntry, ServiceInstanceConfig};

    let services_dir = project_dir.join("services");
    let Ok(entries) = std::fs::read_dir(&services_dir) else { return; };

    for entry in entries.flatten() {
        let fp = entry.path();
        let is_svc_toml = fp.extension().and_then(|e| e.to_str()) == Some("toml")
            && fp.file_stem().and_then(|s| s.to_str())
                .map(|s| s.ends_with(".service"))
                .unwrap_or(false);
        if !is_svc_toml { continue; }

        let stem = fp.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let name = stem.strip_suffix(".service").unwrap_or(stem).to_string();

        let Ok(svc_config) = ServiceInstanceConfig::load(&fp) else { continue; };
        let m = &svc_config.service;

        // Insert a new entry or update the existing one with the standalone file's data.
        // Using entry().or_insert_with() then updating ensures both code paths are covered.
        let se = config.load.services.entry(name).or_insert_with(|| ServiceEntry {
            service_class: m.service_class.clone(),
            alias:         m.alias.clone(),
            host:          m.host.clone(),
            subdomain:     m.subdomain.clone(),
            port:          m.port,
            version:       m.version.clone(),
            tags:          m.tags.clone(),
            env:           Default::default(),
            vars:          svc_config.vars.clone(),
        });
        // Always sync from standalone file so edited vars / subdomain / port is reflected.
        se.service_class = m.service_class.clone();
        se.alias         = m.alias.clone();
        se.host          = m.host.clone();
        se.subdomain     = m.subdomain.clone();
        se.port          = m.port;
        se.version       = m.version.clone();
        se.tags          = m.tags.clone();
        se.vars          = svc_config.vars.clone();
    }
}

/// Load all `.host.toml` files from a project directory.
///
/// Returns `(valid_hosts, load_errors)` — broken files are skipped and
/// their error messages collected for UI notification.
pub fn load_hosts(project_dir: &Path) -> (Vec<HostHandle>, Vec<String>) {
    let mut hosts  = Vec::new();
    let mut errors = Vec::new();
    let Ok(entries) = std::fs::read_dir(project_dir) else { return (hosts, errors); };
    for entry in entries.flatten() {
        let fp = entry.path();
        let is_host_toml = fp.extension().and_then(|e| e.to_str()) == Some("toml")
            && fp.file_stem().and_then(|s| s.to_str())
                .map(|s| s.ends_with(".host"))
                .unwrap_or(false);
        if !is_host_toml { continue; }
        let stem = fp.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let slug = stem.strip_suffix(".host").unwrap_or(stem).to_string();
        match fsn_core::config::host::HostConfig::load(&fp) {
            Ok(config) => { hosts.push(HostHandle { slug, toml_path: fp, config }); }
            Err(e)     => { errors.push(format!("Broken host file '{}': {e}", fp.display())); }
        }
    }
    (hosts, errors)
}

/// Load all `.service.toml` files from `{project_dir}/services/`.
pub fn load_service_instances(project_dir: &Path) -> Vec<ServiceHandle> {
    let services_dir = project_dir.join("services");
    let mut handles = Vec::new();
    let Ok(entries) = std::fs::read_dir(&services_dir) else { return handles; };
    for entry in entries.flatten() {
        let fp = entry.path();
        let is_svc_toml = fp.extension().and_then(|e| e.to_str()) == Some("toml")
            && fp.file_stem().and_then(|s| s.to_str())
                .map(|s| s.ends_with(".service"))
                .unwrap_or(false);
        if !is_svc_toml { continue; }
        let stem = fp.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let name = stem.strip_suffix(".service").unwrap_or(stem).to_string();
        if let Ok(config) = fsn_core::config::project::ServiceInstanceConfig::load(&fp) {
            handles.push(ServiceHandle { name, toml_path: fp, config });
        }
    }
    handles
}

/// Returns true if any `*.project.toml` exists under `root/projects/`.
fn project_toml_exists(root: &Path) -> bool {
    let projects_dir = root.join("projects");
    if !projects_dir.exists() { return false; }
    let Ok(entries) = std::fs::read_dir(&projects_dir) else { return false; };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() { continue; }
        let Ok(inner) = std::fs::read_dir(&path) else { continue; };
        for f in inner.flatten() {
            let fp = f.path();
            if fp.extension().and_then(|e| e.to_str()) == Some("toml")
                && fp.file_stem().and_then(|s| s.to_str())
                    .map(|s| s.ends_with(".project"))
                    .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}
