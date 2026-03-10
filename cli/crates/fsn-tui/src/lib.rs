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
pub mod deploy_thread;
pub mod events;
pub mod events_dashboard;
pub mod handles;
pub mod host_form;
pub mod i18n;
pub mod project_form;
pub mod resource_form;
pub mod schema_form;
pub mod service_form;
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
    let sysinfo  = SysInfo::collect();
    let projects = load_projects(root);
    let mut state = AppState::new(sysinfo, projects);

    // Load the bundled store index (offline — no HTTP required at startup).
    let store_index = fsn_engine::store::StoreClient::load_bundled(&root.join("modules"));
    state.store_entries = store_index.modules;

    // Load hosts for the first selected project.
    if let Some(proj) = state.projects.first() {
        let project_dir = root.join("projects").join(&proj.slug);
        state.hosts = load_hosts(&project_dir);
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

    // Start background reconciler (polls Podman every 5 seconds).
    // The receiver lives in AppState so the rat-salsa Tick handler can drain it.
    state.reconcile_rx = Some(spawn_reconciler(Duration::from_secs(5)));

    app::run_salsa(root.to_path_buf(), &mut state)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Load all projects from `root/projects/` using `ProjectConfig::load()`.
pub fn load_projects(root: &Path) -> Vec<ProjectHandle> {
    let projects_dir = root.join("projects");
    if !projects_dir.exists() { return vec![]; }

    let mut projects = Vec::new();
    let Ok(entries) = std::fs::read_dir(&projects_dir) else { return projects; };

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

            if let Ok(config) = fsn_core::config::project::ProjectConfig::load(&fp) {
                projects.push(ProjectHandle { slug, toml_path: fp, config });
            }
        }
    }
    projects
}

/// Load all `.host.toml` files from a project directory.
pub fn load_hosts(project_dir: &Path) -> Vec<HostHandle> {
    let mut hosts = Vec::new();
    let Ok(entries) = std::fs::read_dir(project_dir) else { return hosts; };
    for entry in entries.flatten() {
        let fp = entry.path();
        let is_host_toml = fp.extension().and_then(|e| e.to_str()) == Some("toml")
            && fp.file_stem().and_then(|s| s.to_str())
                .map(|s| s.ends_with(".host"))
                .unwrap_or(false);
        if !is_host_toml { continue; }
        let stem = fp.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let slug = stem.strip_suffix(".host").unwrap_or(stem).to_string();
        if let Ok(config) = fsn_core::config::host::HostConfig::load(&fp) {
            hosts.push(HostHandle { slug, toml_path: fp, config });
        }
    }
    hosts
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
