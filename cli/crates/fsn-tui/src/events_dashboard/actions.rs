// Sidebar and context action execution.
//
// Pattern: OOP — behaviour lives on the type, not in standalone functions.
//
// Two impl blocks extend types defined elsewhere (avoids circular deps):
//   SidebarItem::open_edit_form — needs form builders (project_form etc.)
//   ConfirmAction::execute      — needs actions.rs CRUD functions
//
// Both are placed here because events_dashboard is the one module that imports
// both the app types AND the form/action helpers.
//
// Public entry points:
//   activate_sidebar_item()       — single source of truth for "Enter on sidebar item"
//   sidebar_start_resource()      — 's' key handler in sidebar focus
//   sidebar_confirm_delete()      — 'x'/Del key handler in sidebar focus
//   handle_new_resource_overlay() — NewResource overlay key handler (pub(crate))

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{
    AppState, ConfirmAction, NotifKind, OverlayLayer, ResourceKind, Screen,
    SidebarAction, SidebarItem, NEW_RESOURCE_ITEMS,
};
use crate::actions::{
    delete_selected_project, delete_selected_host, delete_service_by_name,
    start_service, stop_service_container,
};
use crate::deploy_thread::trigger_deploy;

// ── SidebarItem::open_edit_form — OOP method, second impl block ───────────────
//
// This impl block extends SidebarItem (defined in app/sidebar.rs) with
// form-opening logic that requires access to form builder functions
// (project_form, host_form, service_form).
// Placing it here avoids circular deps: app/ does not import form builders.

impl SidebarItem {
    /// Open the edit form for this resource item.
    ///
    /// Noop for `Section` and `Action` variants — they have no edit form.
    /// Called by `execute_context_action` (mouse.rs) and keyboard 'e' / Enter.
    pub(crate) fn open_edit_form(&self, state: &mut AppState, root: &Path) {
        match self {
            SidebarItem::Project { slug, .. } => {
                if let Some(proj) = state.projects.iter().find(|p| p.slug == *slug).cloned() {
                    let form = crate::project_form::edit_project_form(
                        &proj, &state.svc_handles, &state.store_entries, &state.available_langs,
                    );
                    state.open_form(form);
                }
            }
            SidebarItem::Host { slug, .. } => {
                if let Some(host) = state.hosts.iter().find(|h| h.slug == *slug).cloned() {
                    let slugs = project_slugs(state);
                    let form  = crate::host_form::edit_host_form(&host, slugs);
                    state.open_form(form);
                }
            }
            SidebarItem::Service { name, .. } => {
                // Search all projects for this service (sidebar shows ALL services).
                let proj = state.projects.iter().find(|p| p.config.load.services.contains_key(name)).cloned();
                if let Some(proj) = proj {
                    if let Some(entry) = proj.config.load.services.get(name).cloned() {
                        let slug = crate::resource_form::slugify(name);
                        // Load standalone .service.toml to get vars + vars_comments.
                        let svc_path = root.join("projects")
                            .join(&proj.slug)
                            .join("services")
                            .join(format!("{slug}.service.toml"));
                        let svc_config = fsn_core::config::project::ServiceInstanceConfig::load(&svc_path).ok();
                        let form = crate::service_form::edit_service_form(
                            name, &entry, svc_config.as_ref(), slug,
                            project_slugs(state), host_slugs(state), &proj.slug,
                        );
                        state.open_form(form);
                    }
                }
            }
            _ => {}
        }
    }
}

// ── activate_sidebar_item ─────────────────────────────────────────────────────

/// Activate a sidebar item — the single source of truth for "what happens when
/// an item is selected".
///
/// For Action items: opens the corresponding create form or wizard.
/// For resource items (Project, Host, Service): opens the edit form.
pub fn activate_sidebar_item(item: SidebarItem, state: &mut AppState, root: &Path) {
    match item {
        SidebarItem::Action { kind: SidebarAction::NewProject, .. } => {
            let form = crate::project_form::new_project_form(&state.svc_handles, &state.store_entries, &state.available_langs);
            state.open_form(form);
        }
        SidebarItem::Action { kind: SidebarAction::NewHost, .. } => {
            let slugs   = project_slugs(state);
            let current = current_project_slug(state).to_string();
            let form    = crate::host_form::new_host_form(slugs, &current);
            state.open_form(form);
        }
        SidebarItem::Action { kind: SidebarAction::NewService, .. } => {
            let p_slugs = project_slugs(state);
            let h_slugs = host_slugs(state);
            let cur_p   = current_project_slug(state).to_string();
            let cur_h   = current_host_slug(state).to_string();
            state.open_form(crate::service_form::new_service_form_from_store(
                &state.store_entries, p_slugs, h_slugs, &cur_p, &cur_h,
            ));
        }
        // Project items: check for missing required resources and auto-queue setup forms.
        SidebarItem::Project { .. } => {
            if state.form_queue.is_none() {
                let tasks = collect_missing_tasks(state);
                if !tasks.is_empty() {
                    let first = tasks[0].build_form(state);
                    let mut queue = crate::form_queue::FormQueue::single(first);
                    for kind in tasks.into_iter().skip(1) {
                        let form = kind.build_form(state);
                        queue.push(form, Some(kind));
                    }
                    state.form_queue = Some(queue);
                    state.screen = Screen::NewProject;
                    return;
                }
            }
            // No missing resources — open edit form.
            item.open_edit_form(state, root);
        }
        // Resource items: open their edit form (same behavior as 'e' key).
        other => other.open_edit_form(state, root),
    }
    let _ = root;
}

// ── Sidebar 's' and 'x' handlers ─────────────────────────────────────────────

pub(super) fn sidebar_start_resource(state: &mut AppState, root: &Path) {
    let item = state.current_sidebar_item().cloned();
    match item {
        Some(SidebarItem::Project { slug, .. }) => {
            if let Some(proj) = state.projects.iter().find(|p| p.slug == slug).cloned() {
                let host = state.hosts.first().map(|h| h.config.clone());
                trigger_deploy(state, root, proj, host);
            }
        }
        Some(SidebarItem::Host { slug, .. }) => {
            if let Some(proj) = state.projects.get(state.selected_project).cloned() {
                let host_cfg = state.hosts.iter()
                    .find(|h| h.slug == slug)
                    .map(|h| h.config.clone());
                trigger_deploy(state, root, proj, host_cfg);
            }
        }
        Some(SidebarItem::Service { name, .. }) => {
            start_service(state, &name);
        }
        _ => {}
    }
}

pub(super) fn sidebar_confirm_delete(state: &mut AppState) {
    // Guard: do not offer delete when there is nothing to delete.
    if let Some(SidebarItem::Project { .. }) = state.current_sidebar_item() {
        if state.projects.is_empty() { return; }
    }
    if let Some(item) = state.current_sidebar_item().cloned() {
        // Single source of truth: SidebarItem::delete_confirm() (app/sidebar.rs).
        if let Some((message, data, yes_action)) = item.delete_confirm() {
            state.push_overlay(OverlayLayer::Confirm { message, data, yes_action });
        }
    }
}

// ── ConfirmAction::execute — OOP execution (second impl block) ────────────────
//
// Placed here (not in app/) to avoid circular deps: app/ does not import
// actions.rs, but events_dashboard does.

impl ConfirmAction {
    /// Execute the confirmed action, consuming self.
    pub(crate) fn execute(
        self,
        state: &mut AppState,
        root:  &Path,
        data:  Option<String>,
    ) -> Result<()> {
        match self {
            ConfirmAction::DeleteProject => {
                delete_selected_project(state, root)?;
                state.push_notif(NotifKind::Success, "Project deleted");
            }
            ConfirmAction::DeleteHost => {
                delete_selected_host(state, root)?;
                state.push_notif(NotifKind::Success, "Host deleted");
            }
            ConfirmAction::LeaveForm => {
                state.close_form_queue();
            }
            ConfirmAction::Quit => {
                state.should_quit = true;
            }
            ConfirmAction::DeleteService => {
                let name = data.unwrap_or_default();
                delete_service_by_name(state, root, name.clone())?;
                state.push_notif(NotifKind::Success, format!("Service '{}' deleted", name));
            }
            ConfirmAction::StopService => {
                let name = data.unwrap_or_default();
                stop_service_container(state, name.clone());
                state.push_notif(NotifKind::Info, format!("Service '{}' stopped", name));
            }
            ConfirmAction::MarkModuleInstalled => {
                let id = data.unwrap_or_default();
                state.settings.mark_installed(&id);
                let _ = state.settings.save();
                state.push_notif(NotifKind::Success, format!("Module '{}' marked as installed", id));
            }
            ConfirmAction::MarkModuleUninstalled => {
                let id = data.unwrap_or_default();
                state.settings.mark_uninstalled(&id);
                let _ = state.settings.save();
                state.push_notif(NotifKind::Info, format!("Module '{}' uninstalled", id));
            }
        }
        Ok(())
    }
}

// ── New-resource overlay helpers ──────────────────────────────────────────────

pub fn handle_new_resource_overlay(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let count = NEW_RESOURCE_ITEMS.len();
    match key.code {
        KeyCode::Esc => { state.pop_overlay(); }

        // Circular navigation: Up wraps from 0 → last, Down wraps from last → 0.
        KeyCode::Up => {
            if let Some(OverlayLayer::NewResource { selected }) = state.top_overlay_mut() {
                *selected = selected.checked_sub(1).unwrap_or(count - 1);
            }
        }
        KeyCode::Down => {
            if let Some(OverlayLayer::NewResource { selected }) = state.top_overlay_mut() {
                *selected = (*selected + 1) % count;
            }
        }
        KeyCode::Enter => {
            let idx = match state.top_overlay() {
                Some(OverlayLayer::NewResource { selected }) => *selected,
                _ => return Ok(()),
            };
            state.pop_overlay();
            open_new_resource_form(idx, state, root);
        }
        _ => {}
    }
    Ok(())
}

fn open_new_resource_form(item_idx: usize, state: &mut AppState, root: &Path) {
    let Some(&(_, kind)) = NEW_RESOURCE_ITEMS.get(item_idx) else { return };
    match kind {
        ResourceKind::Project => {
            let form = crate::project_form::new_project_form(&state.svc_handles, &state.store_entries, &state.available_langs);
            state.open_form(form);
        }
        ResourceKind::Host => {
            let slugs   = project_slugs(state);
            let current = current_project_slug(state).to_string();
            state.open_form(crate::host_form::new_host_form(slugs, &current));
        }
        ResourceKind::Service => {
            let p_slugs = project_slugs(state);
            let h_slugs = host_slugs(state);
            let cur_p   = current_project_slug(state).to_string();
            let cur_h   = current_host_slug(state).to_string();
            state.open_form(crate::service_form::new_service_form_from_store(
                &state.store_entries, p_slugs, h_slugs, &cur_p, &cur_h,
            ));
        }
        ResourceKind::Bot => {
            let p_slugs = project_slugs(state);
            let cur_p   = current_project_slug(state).to_string();
            state.open_form(crate::bot_form::new_bot_form(p_slugs, &cur_p));
        }
        ResourceKind::Store => {
            // Stores are edited from Settings, not from the new-resource menu.
        }
    }
    let _ = root;
}

// ── Small helpers ─────────────────────────────────────────────────────────────

/// Collect all project slugs — used when building form dropdowns.
pub(crate) fn project_slugs(state: &AppState) -> Vec<String> {
    state.projects.iter().map(|p| p.slug.clone()).collect()
}

/// Collect all host slugs — used when building form dropdowns.
pub(crate) fn host_slugs(state: &AppState) -> Vec<String> {
    state.hosts.iter().map(|h| h.slug.clone()).collect()
}

/// Slug of the currently selected project, or empty string.
pub(crate) fn current_project_slug(state: &AppState) -> &str {
    state.projects.get(state.selected_project)
        .map(|p| p.slug.as_str())
        .unwrap_or("")
}

/// Slug of the first host, or empty string.
pub(crate) fn current_host_slug(state: &AppState) -> &str {
    state.hosts.get(state.selected_host)
        .map(|h| h.slug.as_str())
        .unwrap_or("")
}

/// Check the current project's required resources and return TaskKind entries
/// for any that are not yet configured.
fn collect_missing_tasks(state: &AppState) -> Vec<crate::task_queue::TaskKind> {
    use crate::task_queue::{DependencyKind, TaskKind};
    let proj = match state.projects.get(state.selected_project) {
        Some(p) => p,
        None    => return vec![],
    };
    let slug = proj.slug.clone();

    let mut tasks = Vec::new();

    // No host configured → queue NewHost
    if state.hosts.is_empty() {
        tasks.push(TaskKind::NewHost { for_project: slug.clone() });
    }

    // No proxy service → queue NewProxy
    if !TaskKind::dep_fulfilled(DependencyKind::Proxy, state) {
        tasks.push(TaskKind::NewProxy { for_host: String::new() });
    }

    tasks
}
