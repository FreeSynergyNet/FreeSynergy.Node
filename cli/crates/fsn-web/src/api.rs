// REST API – tree navigation endpoints.
//
// Routes:
//   GET  /api/v1/project                     → project meta + all services
//   GET  /api/v1/project/service/:name       → single service detail
//   POST /api/v1/project/service/:name/start
//   POST /api/v1/project/service/:name/stop
//   POST /api/v1/project/service/:name/restart
//   GET  /api/v1/hosts                       → host list
//   GET  /api/v1/status                      → legacy flat list (kept for compat)
//   GET  /api/setup/requirements             → setup wizard fields

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use fsn_core::config::service::FieldType;
use fsn_podman::systemd::{self, UnitStatus};
use serde::Serialize;
use std::sync::Arc;

// ── Shared application state ──────────────────────────────────────────────────

/// Passed to every handler via axum State extractor.
#[derive(Clone)]
pub struct AppState {
    pub fsn_root: Arc<std::path::PathBuf>,
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ServiceInfo {
    pub name:            String,
    pub state:           String,    // running | stopped | failed | missing
    pub health:          String,    // healthy | unhealthy | starting | unknown
    pub class_key:       String,
    pub image:           String,
    pub version:         String,
    pub service_domain:  String,
    pub sub_services:     Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ProjectInfo {
    pub name:     String,
    pub domain:   String,
    pub services: Vec<ServiceInfo>,
}

#[derive(Debug, Serialize)]
pub struct HostInfo {
    pub name: String,
    pub ip:   String,
}

#[derive(Debug, Serialize)]
pub struct SetupRequirementJson {
    pub instance_name:  String,
    pub class_key:      String,
    pub key:            String,
    pub label:          String,
    pub description:    Option<String>,
    pub field_type:     String,
    pub auto_generate:  bool,
    pub default:        Option<String>,
    pub options:        Vec<String>,
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn api_routes(state: AppState) -> Router {
    Router::new()
        // Tree navigation (Phase 2)
        .route("/api/v1/project",                              get(get_project))
        .route("/api/v1/project/service/:name",                get(get_service))
        .route("/api/v1/project/service/:name/start",          post(start_service))
        .route("/api/v1/project/service/:name/stop",           post(stop_service))
        .route("/api/v1/project/service/:name/restart",        post(restart_service))
        .route("/api/v1/hosts",                                get(get_hosts))
        // Setup wizard
        .route("/api/setup/requirements",                      get(setup_requirements))
        // Legacy (kept for compat with existing WebUI HTML)
        .route("/api/status",                                  get(status_legacy))
        .route("/api/restart/:name",                           post(restart_legacy))
        .route("/api/stop/:name",                              post(stop_legacy))
        .route("/api/start/:name",                             post(start_legacy))
        .with_state(state)
}

// ── Tree navigation handlers ──────────────────────────────────────────────────

/// GET /api/v1/project
/// Returns project meta and all services with their current state.
async fn get_project(State(s): State<AppState>) -> impl IntoResponse {
    let (name, domain, services) = load_project_services(&s).await;
    Json(ProjectInfo { name, domain, services })
}

/// GET /api/v1/project/service/:name
async fn get_service(
    State(s):   State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let (_proj, _domain, services) = load_project_services(&s).await;
    let svc = services.into_iter().find(|sv| sv.name == name);
    Json(svc)
}

/// POST /api/v1/project/service/:name/start
async fn start_service(Path(name): Path<String>) -> impl IntoResponse {
    action_result(systemd::start(&format!("{}.service", name)).await)
}

/// POST /api/v1/project/service/:name/stop
async fn stop_service(Path(name): Path<String>) -> impl IntoResponse {
    action_result(systemd::stop(&format!("{}.service", name)).await)
}

/// POST /api/v1/project/service/:name/restart
async fn restart_service(Path(name): Path<String>) -> impl IntoResponse {
    let unit = format!("{}.service", name);
    let r = systemd::stop(&unit).await.and(systemd::start(&unit).await);
    action_result(r)
}

/// GET /api/v1/hosts
async fn get_hosts(State(s): State<AppState>) -> impl IntoResponse {
    let hosts = load_hosts(&s);
    Json(hosts)
}

/// GET /api/setup/requirements
async fn setup_requirements(State(s): State<AppState>) -> impl IntoResponse {
    let reqs = load_setup_requirements(&s).await;
    Json(reqs)
}

// ── Legacy handlers (backward compat) ────────────────────────────────────────

async fn status_legacy() -> impl IntoResponse {
    let units = systemd::list_fsn_units().await.unwrap_or_default();
    let mut out = Vec::new();
    for unit in &units {
        let name = unit.trim_end_matches(".service").to_string();
        let state = unit_state_str(systemd::status(unit).await);
        out.push(serde_json::json!({ "name": name, "state": state }));
    }
    Json(out)
}

async fn restart_legacy(Path(name): Path<String>) -> impl IntoResponse {
    let unit = format!("{}.service", name);
    action_result(systemd::stop(&unit).await.and(systemd::start(&unit).await))
}

async fn stop_legacy(Path(name): Path<String>) -> impl IntoResponse {
    action_result(systemd::stop(&format!("{}.service", name)).await)
}

async fn start_legacy(Path(name): Path<String>) -> impl IntoResponse {
    action_result(systemd::start(&format!("{}.service", name)).await)
}

// ── Data loading helpers ──────────────────────────────────────────────────────

async fn load_project_services(s: &AppState) -> (String, String, Vec<ServiceInfo>) {
    // Try to load project config; fall back to live systemd list on error
    let root = s.fsn_root.as_ref();

    // Load from systemd (always works even without config files)
    let units = systemd::list_fsn_units().await.unwrap_or_default();
    let mut services = Vec::new();

    for unit in &units {
        let name  = unit.trim_end_matches(".service").to_string();
        let state = unit_state_str(systemd::status(unit).await);
        services.push(ServiceInfo {
            name:           name.clone(),
            state,
            health:         "unknown".to_string(),
            class_key:      String::new(),
            image:          String::new(),
            version:        read_version_marker(&name, root),
            service_domain: String::new(),
            sub_services:    Vec::new(),
        });
    }

    // Try to enrich with project config data
    if let Some((proj, enriched)) = try_load_project_config(root).await {
        return (proj.0, proj.1, enriched);
    }

    ("(unknown)".to_string(), "(unknown)".to_string(), services)
}

async fn try_load_project_config(
    root: &std::path::Path,
) -> Option<((String, String), Vec<ServiceInfo>)> {
    use fsn_core::config::{HostConfig, ServiceRegistry, ProjectConfig, VaultConfig, resolve_plugins_dir};
    use fsn_engine::{observe::observe, resolve::resolve_desired};

    let proj_path = super::find_project_file(root)?;
    let host_path = super::find_host_file(root)?;
    let proj      = ProjectConfig::load(&proj_path).ok()?;
    let host      = HostConfig::load(&host_path).ok()?;
    let registry  = ServiceRegistry::load(&resolve_plugins_dir(root)).ok()?;
    let vault_pass = std::env::var("FSN_VAULT_PASS").ok();
    let vault = VaultConfig::load(
        proj_path.parent().unwrap_or(root),
        vault_pass.as_deref(),
    ).unwrap_or_default();

    let desired  = resolve_desired(&proj, &host, &registry, &vault, None).ok()?;
    let actual   = observe().await.unwrap_or_default();

    let services = desired.services.iter().map(|inst| {
        let status = actual.find(&inst.name);
        ServiceInfo {
            name:          inst.name.clone(),
            state:         status.map(|s| s.state.to_string()).unwrap_or_else(|| "missing".into()),
            health:        status.map(|s| s.health.to_string()).unwrap_or_else(|| "unknown".into()),
            class_key:     inst.class_key.clone(),
            image:         format!("{}:{}", inst.class.container.image, inst.class.container.image_tag),
            version:       inst.version.clone(),
            service_domain: inst.service_domain.clone(),
            sub_services:   inst.sub_services.iter().map(|s| s.name.clone()).collect(),
        }
    }).collect();

    Some(((proj.project.meta.name, proj.project.domain), services))
}

fn load_hosts(s: &AppState) -> Vec<HostInfo> {
    use fsn_core::config::HostConfig;
    let hosts_dir = s.fsn_root.join("hosts");
    let Ok(entries) = std::fs::read_dir(&hosts_dir) else { return Vec::new() };

    entries.flatten()
        .filter(|e| {
            let n = e.file_name();
            let name = n.to_string_lossy();
            name.ends_with(".host.toml") && name != "example.host.toml"
        })
        .filter_map(|e| {
            let cfg = HostConfig::load(&e.path()).ok()?;
            Some(HostInfo { name: cfg.host.meta.name, ip: cfg.host.ip })
        })
        .collect()
}

async fn load_setup_requirements(s: &AppState) -> Vec<SetupRequirementJson> {
    use fsn_core::config::{HostConfig, ServiceRegistry, ProjectConfig, VaultConfig, resolve_plugins_dir};
    use fsn_engine::{resolve::resolve_desired, setup::collect_requirements};

    let root = s.fsn_root.as_ref();
    let Some(proj_path) = super::find_project_file(root) else { return Vec::new() };
    let Some(host_path) = super::find_host_file(root)    else { return Vec::new() };
    let Ok(proj)     = ProjectConfig::load(&proj_path)               else { return Vec::new() };
    let Ok(host)     = HostConfig::load(&host_path)                  else { return Vec::new() };
    let Ok(registry) = ServiceRegistry::load(&resolve_plugins_dir(root)) else { return Vec::new() };
    let vault_pass = std::env::var("FSN_VAULT_PASS").ok();
    let vault = VaultConfig::load(
        proj_path.parent().unwrap_or(root),
        vault_pass.as_deref(),
    ).unwrap_or_default();
    let Ok(desired) = resolve_desired(&proj, &host, &registry, &vault, None) else { return Vec::new() };

    collect_requirements(&desired)
        .into_iter()
        .map(|r| SetupRequirementJson {
            instance_name: r.instance_name,
            class_key:     r.class_key,
            key:           r.field.key,
            label:         r.field.label,
            description:   r.field.description,
            field_type:    field_type_str(&r.field.field_type).to_string(),
            auto_generate: r.field.auto_generate,
            default:       r.field.default,
            options:       r.field.options,
        })
        .collect()
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn action_result(r: anyhow::Result<()>) -> Json<serde_json::Value> {
    match r {
        Ok(())  => Json(serde_json::json!({"ok": true})),
        Err(e)  => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

fn unit_state_str(r: anyhow::Result<UnitStatus>) -> String {
    match r {
        Ok(UnitStatus::Active)   => "running",
        Ok(UnitStatus::Inactive) => "stopped",
        Ok(UnitStatus::Failed)   => "failed",
        Ok(UnitStatus::NotFound) => "missing",
        Err(_)                   => "error",
    }.to_string()
}

fn field_type_str(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::String => "string",
        FieldType::Secret => "secret",
        FieldType::Email  => "email",
        FieldType::Ip     => "ip",
        FieldType::Select => "select",
        FieldType::Bool   => "bool",
    }
}

fn read_version_marker(name: &str, _root: &std::path::Path) -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let path = std::path::PathBuf::from(home)
        .join(".local/share/fsn/deployed")
        .join(format!("{}.version", name));
    std::fs::read_to_string(path).unwrap_or_default()
}
