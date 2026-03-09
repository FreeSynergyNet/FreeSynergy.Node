// CRUD operations and Podman helpers.
//
// Handles deletion and state reload for projects, hosts, and services.
// Also provides podman_status() and fetch_logs() for the dashboard.

use std::path::Path;

use anyhow::Result;

use crate::app::{AppState, NotifKind, RunState, Screen, SidebarItem};

// ── Project / host / service deletion ────────────────────────────────────────

pub fn delete_selected_project(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project) else { return Ok(()); };
    let project_dir = root.join("projects").join(&proj.slug);
    if let Err(e) = std::fs::remove_dir_all(&project_dir) {
        state.push_notif(NotifKind::Error, format!("Verzeichnis konnte nicht gelöscht werden: {e}"));
    }
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
        if let Err(e) = std::fs::remove_file(&host_file) {
            state.push_notif(NotifKind::Warning, format!("Host-Datei nicht gefunden: {e}"));
        }
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
    if let Err(e) = std::fs::remove_file(&svc_file) {
        state.push_notif(NotifKind::Warning, format!("Service-Datei nicht gefunden: {e}"));
    }

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
///
/// Uses a simple skip-flag: when a matching section header is encountered the flag
/// is set to true; when any OTHER header appears it resets to false. This means
/// the entire section (key-value pairs + sub-tables) is consumed greedily until
/// the next non-matching header. Sufficient for our flat project.toml structure.
pub fn remove_toml_table_block(content: &str, table_path: &str) -> String {
    let header_exact  = format!("[{table_path}]");
    let header_prefix = format!("[{table_path}.");
    let mut out = String::new();
    let mut skip = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            // Every header line re-evaluates the skip flag — this resets it
            // when we leave the target section and enter an unrelated one.
            skip = trimmed == header_exact || trimmed.starts_with(&header_prefix);
        }
        if !skip {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

// ── Clipboard ─────────────────────────────────────────────────────────────────

/// Copy `text` to the system clipboard and push a success/error toast.
pub fn copy_to_clipboard(state: &mut AppState, text: &str) {
    match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text)) {
        Ok(()) => state.push_notif(NotifKind::Info, format!("Kopiert: {}", text)),
        Err(e) => state.push_notif(NotifKind::Warning, format!("Clipboard-Fehler: {e}")),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_block_removal_removes_exact_section() {
        let input = "[project]\nname = \"foo\"\n\n[load.services.myapp]\nimage = \"nginx\"\n\n[other]\nval = 1\n";
        let result = remove_toml_table_block(input, "load.services.myapp");
        assert!(!result.contains("[load.services.myapp]"), "section should be removed");
        assert!(result.contains("[project]"), "other sections must survive");
        assert!(result.contains("[other]"),   "other sections must survive");
    }

    #[test]
    fn toml_block_removal_removes_sub_tables() {
        let input = "[load.services.myapp]\nimage = \"nginx\"\n\n[load.services.myapp.env]\nKEY = \"val\"\n\n[load.services.other]\nimage = \"redis\"\n";
        let result = remove_toml_table_block(input, "load.services.myapp");
        assert!(!result.contains("[load.services.myapp]"),     "exact section must go");
        assert!(!result.contains("[load.services.myapp.env]"), "sub-section must go");
        assert!(result.contains("[load.services.other]"),      "unrelated service stays");
    }

    #[test]
    fn toml_block_removal_no_op_when_not_found() {
        let input = "[project]\nname = \"foo\"\n";
        let result = remove_toml_table_block(input, "nonexistent");
        assert_eq!(result.trim(), input.trim());
    }
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
