// Keyboard event handling.
//
// Dispatches key events to the active screen or topmost overlay.
// Heavy logic is delegated to focused modules:
//   - submit.rs   — form validation and config persistence
//   - actions.rs  — CRUD operations (delete, stop, reload)
//   - deploy_thread.rs — background deploy/export thread
//   - mouse.rs    — mouse events

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{
    AppState, ConfirmAction, DashFocus, LogsState,
    NEW_RESOURCE_ITEMS, OverlayLayer, ResourceKind, Screen, SidebarAction, SidebarItem,
};
use crate::ui::form_node::FormAction;
use crate::actions::{
    delete_selected_project, delete_selected_host, delete_service_by_name,
    fetch_logs, podman_status, stop_service_container, sync_sidebar_selection,
};
use crate::deploy_thread::trigger_deploy;
use crate::submit::{handle_form_submit, handle_wizard_submit};

pub fn handle(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    state.ctrl_hint = key.modifiers.contains(KeyModifiers::CONTROL);

    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        state.should_quit = true;
        return Ok(());
    }

    if key.code == KeyCode::F(1) {
        state.help_visible = !state.help_visible;
        return Ok(());
    }

    if key.code == KeyCode::Esc && state.help_visible {
        state.help_visible = false;
        return Ok(());
    }

    if state.has_overlay() {
        return handle_overlay(key, state, root);
    }

    match state.screen {
        Screen::Welcome    => handle_welcome(key, state),
        Screen::Dashboard  => handle_dashboard(key, state, root),
        Screen::NewProject => handle_resource_form(key, state, root),
        Screen::TaskWizard => handle_wizard(key, state, root),
        Screen::Settings   => handle_settings(key, state),
    }
}

// ── Overlay layer handler ─────────────────────────────────────────────────────

fn handle_overlay(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let overlay_kind = state.top_overlay().map(|o| match o {
        OverlayLayer::Logs(_)          => "logs",
        OverlayLayer::Confirm { .. }   => "confirm",
        OverlayLayer::Deploy(_)        => "deploy",
        OverlayLayer::NewResource { .. } => "new_resource",
    });

    match overlay_kind {
        Some("logs") => {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => { state.pop_overlay(); }
                KeyCode::Up => {
                    if let Some(logs) = state.logs_overlay_mut() {
                        if logs.scroll > 0 { logs.scroll -= 1; }
                    }
                }
                KeyCode::Down => {
                    if let Some(logs) = state.logs_overlay_mut() {
                        let max = logs.lines.len().saturating_sub(1);
                        if logs.scroll < max { logs.scroll += 1; }
                    }
                }
                _ => {}
            }
        }
        Some("confirm") => {
            let (data, yes_action) = {
                let (_, d, a) = state.confirm_overlay().unwrap();
                (d.map(|s| s.to_string()), a)
            };
            match key.code {
                KeyCode::Char('j') | KeyCode::Char('J')
                | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    state.pop_overlay();
                    match yes_action {
                        ConfirmAction::DeleteProject => delete_selected_project(state, root)?,
                        ConfirmAction::DeleteHost    => delete_selected_host(state, root)?,
                        ConfirmAction::LeaveForm => {
                            state.current_form = None;
                            state.screen = if state.projects.is_empty() {
                                Screen::Welcome
                            } else {
                                Screen::Dashboard
                            };
                        }
                        ConfirmAction::LeaveWizard => {
                            state.task_queue = None;
                            state.screen = Screen::Dashboard;
                        }
                        ConfirmAction::Quit => { state.should_quit = true; }
                        ConfirmAction::DeleteService => {
                            delete_service_by_name(state, root, data.unwrap_or_default())?;
                        }
                        ConfirmAction::StopService => {
                            stop_service_container(state, data.unwrap_or_default());
                        }
                    }
                }
                _ => { state.pop_overlay(); }
            }
        }
        Some("deploy") => {
            let done = state.top_overlay().map(|o| {
                if let OverlayLayer::Deploy(ref d) = o { d.done } else { false }
            }).unwrap_or(false);
            if done && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                state.pop_overlay();
                state.deploy_rx = None;
            }
        }
        Some("new_resource") => {
            handle_new_resource_overlay(key, state, root)?;
        }
        _ => { state.pop_overlay(); }
    }
    Ok(())
}

fn handle_new_resource_overlay(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let count = NEW_RESOURCE_ITEMS.len();

    match key.code {
        KeyCode::Esc => { state.pop_overlay(); }
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
            let queue = crate::task_queue::TaskQueue::new(
                crate::task_queue::TaskKind::NewProject, state,
            );
            state.task_queue = Some(queue);
            state.screen = Screen::TaskWizard;
        }
        ResourceKind::Host => {
            let project_slugs = state.projects.iter().map(|p| p.slug.clone()).collect();
            let current = state.projects.get(state.selected_project)
                .map(|p| p.slug.as_str()).unwrap_or("");
            state.current_form = Some(crate::host_form::new_host_form(project_slugs, current));
            state.screen = Screen::NewProject;
        }
        ResourceKind::Service => {
            state.current_form = Some(crate::service_form::new_service_form());
            state.screen = Screen::NewProject;
        }
        ResourceKind::Bot => {
            state.current_form = Some(crate::bot_form::new_bot_form());
            state.screen = Screen::NewProject;
        }
    }
    let _ = root;
}

// ── Welcome screen ────────────────────────────────────────────────────────────

fn handle_welcome(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match key.code {
        KeyCode::Char('q') => state.should_quit = true,
        KeyCode::Char('l') | KeyCode::Char('L') => state.lang = state.lang.toggle(),
        KeyCode::Left | KeyCode::Right => state.welcome_focus = 1 - state.welcome_focus,
        KeyCode::Enter => {
            if state.welcome_focus == 0 {
                state.current_form = Some(crate::project_form::new_project_form());
                state.screen = Screen::NewProject;
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Generic resource form handler ─────────────────────────────────────────────

fn handle_resource_form(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let action = if let Some(ref mut form) = state.current_form {
        form.handle_key(key)
    } else {
        FormAction::Unhandled
    };

    match action {
        FormAction::Cancel => {
            let dirty = state.current_form.as_ref().map(|f| f.is_dirty()).unwrap_or(false);
            if dirty {
                state.push_overlay(OverlayLayer::Confirm {
                    message:    "form.confirm.leave".into(),
                    data:       None,
                    yes_action: ConfirmAction::LeaveForm,
                });
            } else {
                state.current_form = None;
                state.screen = if state.projects.is_empty() { Screen::Welcome } else { Screen::Dashboard };
            }
        }
        FormAction::LangToggle => state.lang = state.lang.toggle(),
        FormAction::Submit     => handle_form_submit(state, root)?,
        FormAction::Consumed   => {}
        FormAction::Unhandled  => {
            if let KeyCode::Char('l') | KeyCode::Char('L') = key.code {
                state.lang = state.lang.toggle();
            }
        }
        FormAction::FocusNext | FormAction::FocusPrev
        | FormAction::TabNext  | FormAction::TabPrev
        | FormAction::ValueChanged => {}
        FormAction::Quit => state.should_quit = true,
    }
    Ok(())
}

// ── Task wizard ───────────────────────────────────────────────────────────────

fn handle_wizard(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let action = if let Some(ref mut queue) = state.task_queue {
        if let Some(task) = queue.tasks.get_mut(queue.active) {
            if let Some(ref mut form) = task.form { form.handle_key(key) }
            else { FormAction::Unhandled }
        } else { FormAction::Unhandled }
    } else { FormAction::Unhandled };

    match action {
        FormAction::Cancel => confirm_leave_wizard(state),
        FormAction::LangToggle => state.lang = state.lang.toggle(),
        FormAction::Submit     => handle_wizard_submit(state, root)?,
        FormAction::Consumed   => {}
        FormAction::Unhandled  => {
            match key.code {
                KeyCode::Esc => confirm_leave_wizard(state),
                KeyCode::Char('l') | KeyCode::Char('L') => state.lang = state.lang.toggle(),
                _ => {}
            }
        }
        FormAction::FocusNext | FormAction::FocusPrev
        | FormAction::TabNext  | FormAction::TabPrev
        | FormAction::ValueChanged => {}
        FormAction::Quit => state.should_quit = true,
    }
    Ok(())
}

fn confirm_leave_wizard(state: &mut AppState) {
    let dirty = state.task_queue.as_ref().and_then(|q| {
        q.tasks.get(q.active)?.form.as_ref().map(|f| f.is_dirty())
    }).unwrap_or(false);
    if dirty {
        state.push_overlay(OverlayLayer::Confirm {
            message:    "form.confirm.leave".into(),
            data:       None,
            yes_action: ConfirmAction::LeaveWizard,
        });
    } else {
        state.task_queue = None;
        state.screen = Screen::Dashboard;
    }
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

fn handle_dashboard(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    match state.dash_focus {
        DashFocus::Sidebar => handle_dashboard_sidebar(key, state, root),
        DashFocus::Services => handle_dashboard_services(key, state, root),
    }
}

fn handle_dashboard_sidebar(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.push_overlay(OverlayLayer::Confirm {
                message:    "confirm.quit".into(),
                data:       None,
                yes_action: ConfirmAction::Quit,
            });
        }
        KeyCode::Char('L') => state.lang = state.lang.toggle(),
        KeyCode::Tab => state.dash_focus = DashFocus::Services,

        KeyCode::Up => {
            let cur = state.sidebar_cursor;
            let prev = (0..cur).rev().find(|&i| state.sidebar_items[i].is_selectable());
            if let Some(prev) = prev {
                state.sidebar_cursor = prev;
                sync_sidebar_selection(state, root);
            }
        }
        KeyCode::Down => {
            let cur = state.sidebar_cursor;
            let len = state.sidebar_items.len();
            let next = (cur + 1..len).find(|&i| state.sidebar_items[i].is_selectable());
            if let Some(next) = next {
                state.sidebar_cursor = next;
                sync_sidebar_selection(state, root);
            }
        }

        KeyCode::Char('S') => {
            state.settings_cursor = 0;
            state.screen = Screen::Settings;
        }
        KeyCode::Char('n') => {
            state.push_overlay(OverlayLayer::NewResource { selected: 0 });
        }

        KeyCode::Char('e') => {
            open_sidebar_edit_form(state);
        }
        KeyCode::Enter => {
            open_sidebar_action_or_edit(state, root);
        }

        KeyCode::Char('s') => {
            sidebar_start_resource(state, root);
        }
        KeyCode::Char('x') | KeyCode::Delete => {
            sidebar_confirm_delete(state);
        }

        _ => {}
    }
    Ok(())
}

fn handle_dashboard_services(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            state.push_overlay(OverlayLayer::Confirm {
                message:    "confirm.quit".into(),
                data:       None,
                yes_action: ConfirmAction::Quit,
            });
        }
        KeyCode::Char('L') => state.lang = state.lang.toggle(),
        KeyCode::Tab => state.dash_focus = DashFocus::Sidebar,

        KeyCode::Up   => { if state.selected > 0 { state.selected -= 1; } }
        KeyCode::Down => {
            if state.selected + 1 < state.services.len() { state.selected += 1; }
        }

        KeyCode::Char('n') => {
            state.push_overlay(OverlayLayer::NewResource { selected: 0 });
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
                trigger_deploy(state, root, proj.slug.clone(), proj.config.clone(), host);
            }
        }
        KeyCode::Char('r') => {
            if let Some(svc) = state.services.get(state.selected) {
                let _ = std::process::Command::new("podman")
                    .args(["restart", &svc.name]).output();
                if let Some(row) = state.services.get_mut(state.selected) {
                    row.status = podman_status(&row.name);
                }
            }
        }
        KeyCode::Char('x') => {
            if let Some(svc) = state.services.get(state.selected) {
                state.push_overlay(OverlayLayer::Confirm {
                    message:    "confirm.stop.service".into(),
                    data:       Some(svc.name.clone()),
                    yes_action: ConfirmAction::StopService,
                });
            }
        }
        KeyCode::Char('s') => {
            if let Some(svc) = state.services.get(state.selected).cloned() {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "start", &format!("{}.service", svc.name)])
                    .output();
                if let Some(row) = state.services.iter_mut().find(|s| s.name == svc.name) {
                    row.status = podman_status(&svc.name);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Sidebar action helpers ────────────────────────────────────────────────────

fn open_sidebar_edit_form(state: &mut AppState) {
    let item = state.current_sidebar_item().cloned();
    match item {
        Some(SidebarItem::Project { slug, .. }) => {
            if let Some(proj) = state.projects.iter().find(|p| p.slug == slug).cloned() {
                state.current_form = Some(crate::project_form::edit_project_form(&proj));
                state.screen = Screen::NewProject;
            }
        }
        Some(SidebarItem::Host { slug, .. }) => {
            if let Some(host) = state.hosts.iter().find(|h| h.slug == slug).cloned() {
                let project_slugs = state.projects.iter().map(|p| p.slug.clone()).collect();
                state.current_form = Some(crate::host_form::edit_host_form(&host, project_slugs));
                state.screen = Screen::NewProject;
            }
        }
        Some(SidebarItem::Service { name, .. }) => {
            if let Some(proj) = state.projects.get(state.selected_project).cloned() {
                if let Some(entry) = proj.config.load.services.get(&name).cloned() {
                    let slug = crate::resource_form::slugify(&name);
                    state.current_form = Some(crate::service_form::edit_service_form(&name, &entry, slug));
                    state.screen = Screen::NewProject;
                }
            }
        }
        _ => {}
    }
}

fn open_sidebar_action_or_edit(state: &mut AppState, root: &Path) {
    let item = state.current_sidebar_item().cloned();
    match item {
        Some(SidebarItem::Action { kind: SidebarAction::NewProject, .. }) => {
            let queue = crate::task_queue::TaskQueue::new(
                crate::task_queue::TaskKind::NewProject, state,
            );
            state.task_queue = Some(queue);
            state.screen = Screen::TaskWizard;
        }
        Some(SidebarItem::Action { kind: SidebarAction::NewHost, .. }) => {
            let project_slugs = state.projects.iter().map(|p| p.slug.clone()).collect();
            let current = state.projects.get(state.selected_project)
                .map(|p| p.slug.as_str()).unwrap_or("");
            state.current_form = Some(crate::host_form::new_host_form(project_slugs, current));
            state.screen = Screen::NewProject;
        }
        Some(SidebarItem::Action { kind: SidebarAction::NewService, .. }) => {
            state.current_form = Some(crate::service_form::new_service_form());
            state.screen = Screen::NewProject;
        }
        Some(SidebarItem::Project { slug, .. }) => {
            if let Some(proj) = state.projects.iter().find(|p| p.slug == slug).cloned() {
                state.current_form = Some(crate::project_form::edit_project_form(&proj));
                state.screen = Screen::NewProject;
            }
        }
        Some(SidebarItem::Host { slug, .. }) => {
            if let Some(host) = state.hosts.iter().find(|h| h.slug == slug).cloned() {
                let project_slugs = state.projects.iter().map(|p| p.slug.clone()).collect();
                state.current_form = Some(crate::host_form::edit_host_form(&host, project_slugs));
                state.screen = Screen::NewProject;
            }
        }
        Some(SidebarItem::Service { name, .. }) => {
            if let Some(proj) = state.projects.get(state.selected_project).cloned() {
                if let Some(entry) = proj.config.load.services.get(&name).cloned() {
                    let slug = crate::resource_form::slugify(&name);
                    state.current_form = Some(crate::service_form::edit_service_form(&name, &entry, slug));
                    state.screen = Screen::NewProject;
                }
            }
        }
        _ => {}
    }
    let _ = root;
}

fn sidebar_start_resource(state: &mut AppState, root: &Path) {
    let item = state.current_sidebar_item().cloned();
    match item {
        Some(SidebarItem::Project { slug, .. }) => {
            if let Some(proj) = state.projects.iter().find(|p| p.slug == slug).cloned() {
                let host = state.hosts.first().map(|h| h.config.clone());
                trigger_deploy(state, root, proj.slug.clone(), proj.config.clone(), host);
            }
        }
        Some(SidebarItem::Host { slug, .. }) => {
            if let Some(proj) = state.projects.get(state.selected_project).cloned() {
                let host_cfg = state.hosts.iter()
                    .find(|h| h.slug == slug)
                    .map(|h| h.config.clone());
                trigger_deploy(state, root, proj.slug.clone(), proj.config.clone(), host_cfg);
            }
        }
        Some(SidebarItem::Service { name, .. }) => {
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "start", &format!("{}.service", name)])
                .output();
            if let Some(row) = state.services.iter_mut().find(|s| s.name == name) {
                row.status = podman_status(&name);
            }
        }
        _ => {}
    }
}

fn sidebar_confirm_delete(state: &mut AppState) {
    let item = state.current_sidebar_item().cloned();
    match item {
        Some(SidebarItem::Project { .. }) if !state.projects.is_empty() => {
            state.push_overlay(OverlayLayer::Confirm {
                message:    "confirm.delete.project".into(),
                data:       None,
                yes_action: ConfirmAction::DeleteProject,
            });
        }
        Some(SidebarItem::Host { slug, .. }) => {
            state.push_overlay(OverlayLayer::Confirm {
                message:    "confirm.delete.host".into(),
                data:       Some(slug.clone()),
                yes_action: ConfirmAction::DeleteHost,
            });
        }
        Some(SidebarItem::Service { name, .. }) => {
            state.push_overlay(OverlayLayer::Confirm {
                message:    "confirm.delete.service".into(),
                data:       Some(name.clone()),
                yes_action: ConfirmAction::DeleteService,
            });
        }
        _ => {}
    }
}

// ── Settings screen ───────────────────────────────────────────────────────────

fn handle_settings(key: KeyEvent, state: &mut AppState) -> Result<()> {
    use fsn_core::config::StoreConfig;

    let n_stores = state.settings.stores.len();

    match key.code {
        KeyCode::Up => {
            if state.settings_cursor > 0 { state.settings_cursor -= 1; }
        }
        KeyCode::Down => {
            if n_stores > 0 && state.settings_cursor < n_stores - 1 {
                state.settings_cursor += 1;
            }
        }
        KeyCode::Char(' ') => {
            if let Some(store) = state.settings.stores.get_mut(state.settings_cursor) {
                store.enabled = !store.enabled;
                let _ = state.settings.save();
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
            if !state.settings.stores.is_empty() {
                state.settings.stores.remove(state.settings_cursor);
                if state.settings_cursor >= state.settings.stores.len() && state.settings_cursor > 0 {
                    state.settings_cursor -= 1;
                }
                let _ = state.settings.save();
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            state.settings.stores.push(StoreConfig {
                name:    "New Store".into(),
                url:     "https://".into(),
                enabled: false,
            });
            state.settings_cursor = state.settings.stores.len().saturating_sub(1);
            let _ = state.settings.save();
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            state.screen = Screen::Dashboard;
        }
        _ => {}
    }
    Ok(())
}
