// CRUD operations and Podman helpers.
//
// Handles deletion and state reload for projects, hosts, and services.
// Also provides podman_status() and fetch_logs() for the dashboard.

use std::path::Path;

use anyhow::Result;

use crate::app::{AppState, RunState, Screen, SidebarItem};

// ── Project / host / service deletion ────────────────────────────────────────

pub fn delete_selected_project(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project) else { return Ok(()); };
    let project_dir = root.join("projects").join(&proj.slug);
    let _ = std::fs::remove_dir_all(&project_dir);
    state.projects.remove(state.selected_project);
    if state.selected_project > 0 && state.selected_project >= state.projects.len() {
        state.selected_project -= 1;
    }
    state.hosts.clear();
    state.rebuild_sidebar();
    state.rebuild_services();
    if state.projects.is_empty() { state.screen = Screen::Welcome; }
    Ok(())
}

pub fn delete_selected_host(state: &mut AppState, root: &Path) -> Result<()> {
    let slug = match state.hosts.get(state.selected_host) {
        Some(h) => h.slug.clone(),
        None    => return Ok(()),
    };
    if let Some(proj) = state.projects.get(state.selected_project) {
        let host_file = root
            .join("projects")
            .join(&proj.slug)
            .join(format!("{}.host.toml", slug));
        let _ = std::fs::remove_file(&host_file);
    }
    state.hosts.remove(state.selected_host);
    if state.selected_host > 0 && state.selected_host >= state.hosts.len() {
        state.selected_host -= 1;
    }
    state.rebuild_sidebar();
    Ok(())
}

pub fn delete_service_by_name(state: &mut AppState, root: &Path, name: String) -> Result<()> {
    if name.is_empty() { return Ok(()); }

    let Some(proj) = state.projects.get(state.selected_project).cloned() else { return Ok(()); };
    let project_dir  = root.join("projects").join(&proj.slug);
    let services_dir = project_dir.join("services");
    let slug         = crate::resource_form::slugify(&name);

    let svc_file = services_dir.join(format!("{slug}.service.toml"));
    let _ = std::fs::remove_file(&svc_file);

    if let Ok(content) = std::fs::read_to_string(&proj.toml_path) {
        let filtered = remove_toml_table_block(&content, &format!("load.services.{slug}"));
        let _ = std::fs::write(&proj.toml_path, filtered);
    }

    state.projects = crate::load_projects(root);
    state.rebuild_services();
    state.rebuild_sidebar();
    Ok(())
}

/// Stop a running container without deleting its config.
pub fn stop_service_container(state: &mut AppState, name: String) {
    if name.is_empty() { return; }
    let _ = std::process::Command::new("podman").args(["stop", &name]).output();
    let _ = std::process::Command::new("podman").args(["rm",   &name]).output();
    if let Some(row) = state.services.iter_mut().find(|s| s.name == name) {
        row.status = podman_status(&name);
    }
}

// ── Sidebar sync ──────────────────────────────────────────────────────────────

pub fn reload_hosts(state: &mut AppState, root: &Path) {
    if let Some(proj) = state.projects.get(state.selected_project) {
        state.hosts = crate::load_hosts(&root.join("projects").join(&proj.slug));
        state.rebuild_sidebar();
    }
}

/// Sync `selected_project` / `selected_host` after `sidebar_cursor` moves.
pub fn sync_sidebar_selection(state: &mut AppState, root: &Path) {
    match state.current_sidebar_item().cloned() {
        Some(SidebarItem::Project { slug, .. }) => {
            if let Some(idx) = state.projects.iter().position(|p| p.slug == slug) {
                if state.selected_project != idx {
                    state.selected_project = idx;
                    reload_hosts(state, root);
                    state.rebuild_services();
                    state.rebuild_sidebar();
                }
            }
        }
        Some(SidebarItem::Host { slug, .. }) => {
            if let Some(idx) = state.hosts.iter().position(|h| h.slug == slug) {
                state.selected_host = idx;
            }
        }
        Some(SidebarItem::Service { .. }) => {
            state.rebuild_services();
        }
        _ => {}
    }
}

// ── TOML helper ───────────────────────────────────────────────────────────────

/// Remove all lines belonging to `[table_path]` and `[table_path.*]` from TOML text.
pub fn remove_toml_table_block(content: &str, table_path: &str) -> String {
    let header_exact  = format!("[{table_path}]");
    let header_prefix = format!("[{table_path}.");
    let mut out = String::new();
    let mut skip = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            skip = trimmed == header_exact || trimmed.starts_with(&header_prefix);
        }
        if !skip {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

// ── Podman helpers ────────────────────────────────────────────────────────────

pub fn podman_status(name: &str) -> RunState {
    let out = std::process::Command::new("podman")
        .args(["inspect", "--format", "{{.State.Status}}", name])
        .output();
    match out {
        Ok(o) => match String::from_utf8_lossy(&o.stdout).trim() {
            "running"            => RunState::Running,
            "exited" | "stopped" => RunState::Stopped,
            "error"              => RunState::Failed,
            _                    => RunState::Missing,
        },
        Err(_) => RunState::Missing,
    }
}

pub fn fetch_logs(name: &str) -> Vec<String> {
    let out = std::process::Command::new("podman")
        .args(["logs", "--tail", "100", name])
        .output();
    match out {
        Ok(o) => {
            let text = if o.stdout.is_empty() { o.stderr } else { o.stdout };
            String::from_utf8_lossy(&text).lines().map(|l| l.to_string()).collect()
        }
        Err(_) => vec!["[Logs nicht verfügbar]".into()],
    }
}
