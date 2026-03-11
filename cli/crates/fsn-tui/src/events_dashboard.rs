// Dashboard keyboard event handling.
//
// Design Pattern: Chain of Responsibility — key events are passed through a
// shared pre-handler (handle_dashboard_shared) before reaching the focus-specific
// handler (sidebar or services). Shared keys (quit, lang-toggle, new-resource)
// are handled once, not duplicated in each branch.
//
// Entry point: handle_dashboard() — called from events.rs Screen::Dashboard arm.
// activate_sidebar_item() is pub(crate) — shared by keyboard and future rat-widget mouse handlers.

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{
    AppState, ConfirmAction, DashFocus, LogsState, NotifKind, OverlayLayer, ResourceKind, Screen,
    SidebarAction, SidebarItem, NEW_RESOURCE_ITEMS,
};
use crate::actions::{
    copy_to_clipboard,
    delete_selected_project, delete_selected_host, delete_service_by_name,
    fetch_logs, restart_service, start_service, stop_service_container,
    sync_sidebar_selection,
};
use crate::deploy_thread::trigger_deploy;

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn handle_dashboard(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    match state.dash_focus {
        DashFocus::Sidebar  => handle_dashboard_sidebar(key, state, root),
        DashFocus::Services => handle_dashboard_services(key, state, root),
    }
}

// ── Sidebar filter ────────────────────────────────────────────────────────────

fn handle_sidebar_filter_key(key: KeyEvent, state: &mut AppState) -> Result<()> {
    use crossterm::event::KeyCode;
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            state.sidebar_filter = None;
        }
        KeyCode::Up => {
            let indices: Vec<usize> = state.visible_sidebar_items().into_iter().map(|(i, _)| i).collect();
            if let Some(pos) = indices.iter().position(|&i| i == state.sidebar_cursor) {
                if pos > 0 { state.sidebar_cursor = indices[pos - 1]; }
            }
        }
        KeyCode::Down => {
            let indices: Vec<usize> = state.visible_sidebar_items().into_iter().map(|(i, _)| i).collect();
            if let Some(pos) = indices.iter().position(|&i| i == state.sidebar_cursor) {
                if pos + 1 < indices.len() { state.sidebar_cursor = indices[pos + 1]; }
            } else if let Some(&first) = indices.first() {
                state.sidebar_cursor = first;
            }
        }
        KeyCode::Backspace => {
            if let Some(ref mut f) = state.sidebar_filter { f.pop(); }
            adjust_cursor_to_filter(state);
        }
        KeyCode::Char(c) => {
            if let Some(ref mut f) = state.sidebar_filter { f.push(c); }
            adjust_cursor_to_filter(state);
        }
        _ => {}
    }
    Ok(())
}

/// After the filter query changes, ensure sidebar_cursor points to a visible item.
fn adjust_cursor_to_filter(state: &mut AppState) {
    let indices: Vec<usize> = state.visible_sidebar_items().into_iter().map(|(i, _)| i).collect();
    if indices.is_empty() { return; }
    if !indices.contains(&state.sidebar_cursor) {
        state.sidebar_cursor = indices[0];
    }
}

// ── Shared dashboard shortcuts ────────────────────────────────────────────────

/// Handle keys that are identical in both sidebar and services focus.
/// Returns `true` if the key was consumed so the caller can return early.
///
/// Shared keys:  q/Esc → quit confirm  |  n → new-resource popup
/// Note: 'L' lang-toggle is handled globally in events.rs before screen dispatch.
fn handle_dashboard_shared(key: KeyEvent, state: &mut AppState) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.push_overlay(OverlayLayer::Confirm {
                message: "confirm.quit".into(), data: None, yes_action: ConfirmAction::Quit,
            });
            true
        }
        KeyCode::Char('n') => {
            // Show the full new-resource picker (all 4 types: Project / Host / Service / Bot).
            // Uses NewResource overlay — rendered by ui/mod.rs::render_new_resource(),
            // handled by handle_new_resource_overlay().
            state.push_overlay(crate::app::OverlayLayer::NewResource { selected: 0 });
            true
        }
        _ => false,
    }
}

// ── Sidebar focus ─────────────────────────────────────────────────────────────

fn handle_dashboard_sidebar(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    // Filter mode intercepts all keys — Esc closes filter, typing refines it.
    if state.sidebar_filter.is_some() {
        return handle_sidebar_filter_key(key, state);
    }

    if handle_dashboard_shared(key, state) { return Ok(()); }

    match key.code {
        KeyCode::Tab => state.dash_focus = DashFocus::Services,

        KeyCode::Up => {
            let cur = state.sidebar_cursor;
            if let Some(prev) = (0..cur).rev().find(|&i| state.sidebar_items[i].is_selectable()) {
                state.sidebar_cursor = prev;
                sync_sidebar_selection(state, root);
            }
        }
        KeyCode::Down => {
            let cur = state.sidebar_cursor;
            let len = state.sidebar_items.len();
            if let Some(next) = (cur + 1..len).find(|&i| state.sidebar_items[i].is_selectable()) {
                state.sidebar_cursor = next;
                sync_sidebar_selection(state, root);
            }
        }

        KeyCode::Char('/') => {
            state.sidebar_filter = Some(String::new());
        }

        KeyCode::Char('S') => {
            state.settings_cursor = 0;
            state.screen = Screen::Settings;
        }

        // 'e' = explicit edit (same as Enter on a resource item, but not on Action items).
        KeyCode::Char('e') => {
            if let Some(item) = state.current_sidebar_item().cloned() {
                item.open_edit_form(state);
            }
        }
        // Enter = "activate": opens create form for Action items, edit form for resources.
        KeyCode::Enter => {
            if let Some(item) = state.current_sidebar_item().cloned() {
                activate_sidebar_item(item, state, root);
            }
        }

        KeyCode::Char('s') => sidebar_start_resource(state, root),
        KeyCode::Char('x') | KeyCode::Delete => sidebar_confirm_delete(state),

        // 'y' = yank (copy) selected item name to clipboard.
        KeyCode::Char('y') => {
            if let Some(item) = state.current_sidebar_item() {
                let text = match item {
                    SidebarItem::Project { name, .. } => name.clone(),
                    SidebarItem::Host    { name, .. } => name.clone(),
                    SidebarItem::Service { name, .. } => name.clone(),
                    _ => String::new(),
                };
                if !text.is_empty() { copy_to_clipboard(state, &text); }
            }
        }

        _ => {}
    }
    Ok(())
}

// ── Services focus ────────────────────────────────────────────────────────────

fn handle_dashboard_services(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    if handle_dashboard_shared(key, state) { return Ok(()); }

    match key.code {
        KeyCode::Tab => state.dash_focus = DashFocus::Sidebar,

        KeyCode::Up   => { if state.selected > 0 { state.selected -= 1; } }
        KeyCode::Down => {
            if state.selected + 1 < state.services.len() { state.selected += 1; }
        }

        // Space = toggle current service in multi-select set.
        KeyCode::Char(' ') => {
            let idx = state.selected;
            if state.selected_services.contains(&idx) {
                state.selected_services.remove(&idx);
            } else {
                state.selected_services.insert(idx);
            }
        }

        // 'u' = clear all selections.
        KeyCode::Char('u') => {
            state.selected_services.clear();
        }

        KeyCode::Char('l') => {
            if let Some(svc) = state.services.get(state.selected) {
                let lines = fetch_logs(&svc.name);
                state.push_overlay(OverlayLayer::Logs(LogsState {
                    service_name: svc.name.clone(), lines, scroll: 0,
                }));
            }
        }
        KeyCode::Char('d') => {
            if let Some(proj) = state.projects.get(state.selected_project).cloned() {
                let host = state.hosts.first().map(|h| h.config.clone());
                trigger_deploy(state, root, proj, host);
            }
        }
        KeyCode::Char('r') => {
            if let Some(name) = state.services.get(state.selected).map(|s| s.name.clone()) {
                restart_service(state, &name);
            }
        }
        KeyCode::Char('x') => {
            if !state.selected_services.is_empty() {
                // Batch stop: stop all selected services immediately (no confirm for batch).
                let names: Vec<String> = state.selected_services.iter()
                    .filter_map(|&i| state.services.get(i).map(|s| s.name.clone()))
                    .collect();
                let count = names.len();
                for name in names {
                    stop_service_container(state, name);
                }
                state.selected_services.clear();
                state.push_notif(NotifKind::Info, format!("{} services stopped", count));
            } else if let Some(svc) = state.services.get(state.selected) {
                state.push_overlay(OverlayLayer::Confirm {
                    message:    "confirm.stop.service".into(),
                    data:       Some(svc.name.clone()),
                    yes_action: ConfirmAction::StopService,
                });
            }
        }
        KeyCode::Char('s') => {
            if !state.selected_services.is_empty() {
                // Batch start: start all selected services.
                let names: Vec<String> = state.selected_services.iter()
                    .filter_map(|&i| state.services.get(i).map(|s| s.name.clone()))
                    .collect();
                let count = names.len();
                for name in names {
                    start_service(state, &name);
                }
                state.selected_services.clear();
                state.push_notif(NotifKind::Info, format!("{} services started", count));
            } else if let Some(name) = state.services.get(state.selected).map(|s| s.name.clone()) {
                start_service(state, &name);
            }
        }

        // 'y' = yank domain of selected service to clipboard.
        KeyCode::Char('y') => {
            if let Some(domain) = state.services.get(state.selected).map(|s| s.domain.clone()) {
                copy_to_clipboard(state, &domain);
            }
        }

        _ => {}
    }
    Ok(())
}

// ── SidebarItem::open_edit_form — OOP method, second impl block ───────────────
//
// Design Pattern: OOP — behaviour lives on the type, not in standalone functions.
//
// This impl block extends SidebarItem (defined in app.rs) with form-opening
// logic that requires access to form builder functions (project_form, host_form,
// service_form).  Placing it here avoids circular deps: app.rs does not import
// form builders, but events_dashboard.rs does.

impl SidebarItem {
    /// Open the edit form for this resource item.
    ///
    /// Noop for `Section` and `Action` variants — they have no edit form.
    /// Called by `execute_context_action` (mouse.rs) and keyboard 'e' / Enter.
    pub(crate) fn open_edit_form(&self, state: &mut AppState) {
        match self {
            SidebarItem::Project { slug, .. } => {
                if let Some(proj) = state.projects.iter().find(|p| p.slug == *slug).cloned() {
                    let form = crate::project_form::edit_project_form(
                        &proj, &state.svc_handles, &state.store_entries,
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
                if let Some(proj) = state.projects.get(state.selected_project).cloned() {
                    if let Some(entry) = proj.config.load.services.get(name).cloned() {
                        let slug = crate::resource_form::slugify(name);
                        let form = crate::service_form::edit_service_form(name, &entry, slug);
                        state.open_form(form);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Activate a sidebar item — the single source of truth for "what happens when
/// an item is selected".
///
/// For Action items: opens the corresponding create form or wizard.
/// For resource items (Project, Host, Service): opens the edit form.
pub(crate) fn activate_sidebar_item(item: SidebarItem, state: &mut AppState, root: &Path) {
    match item {
        SidebarItem::Action { kind: SidebarAction::NewProject, .. } => {
            let form = crate::project_form::new_project_form(&state.svc_handles, &state.store_entries);
            state.open_form(form);
        }
        SidebarItem::Action { kind: SidebarAction::NewHost, .. } => {
            let slugs   = project_slugs(state);
            let current = current_project_slug(state).to_string();
            let form    = crate::host_form::new_host_form(slugs, &current);
            state.open_form(form);
        }
        SidebarItem::Action { kind: SidebarAction::NewService, .. } => {
            state.open_form(crate::service_form::new_service_form());
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
            item.open_edit_form(state);
        }
        // Resource items: open their edit form (same behavior as 'e' key).
        other => other.open_edit_form(state),
    }
    let _ = root;
}

fn sidebar_start_resource(state: &mut AppState, root: &Path) {
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

fn sidebar_confirm_delete(state: &mut AppState) {
    // Guard: do not offer delete when there is nothing to delete.
    if let Some(SidebarItem::Project { .. }) = state.current_sidebar_item() {
        if state.projects.is_empty() { return; }
    }
    if let Some(item) = state.current_sidebar_item().cloned() {
        // Single source of truth: SidebarItem::delete_confirm() (app.rs).
        if let Some((message, data, yes_action)) = item.delete_confirm() {
            state.push_overlay(OverlayLayer::Confirm { message, data, yes_action });
        }
    }
}

// ── New-resource overlay helpers ──────────────────────────────────────────────

pub(crate) fn handle_new_resource_overlay(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
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
            let form = crate::project_form::new_project_form(&state.svc_handles, &state.store_entries);
            state.open_form(form);
        }
        ResourceKind::Host => {
            let slugs   = project_slugs(state);
            let current = current_project_slug(state).to_string();
            state.open_form(crate::host_form::new_host_form(slugs, &current));
        }
        ResourceKind::Service => {
            state.open_form(crate::service_form::new_service_form());
        }
        ResourceKind::Bot => {
            state.open_form(crate::bot_form::new_bot_form());
        }
    }
    let _ = root;
}

// ── ConfirmAction — OOP execution (second impl block) ─────────────────────────
//
// Design Pattern: OOP — behaviour lives on the type, not in standalone functions.
//
// Placed here (not in app.rs) to avoid circular deps: app.rs does not import
// actions.rs, but events_dashboard.rs does.

impl ConfirmAction {
    /// Execute the confirmed action, consuming self.
    pub(crate) fn execute(
        self,
        state: &mut AppState,
        root: &Path,
        data: Option<String>,
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
        }
        Ok(())
    }
}

// ── Small helpers ─────────────────────────────────────────────────────────────

/// Collect all project slugs — used when building host form dropdowns.
fn project_slugs(state: &AppState) -> Vec<String> {
    state.projects.iter().map(|p| p.slug.clone()).collect()
}

/// Slug of the currently selected project, or empty string.
fn current_project_slug(state: &AppState) -> &str {
    state.projects.get(state.selected_project)
        .map(|p| p.slug.as_str())
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
