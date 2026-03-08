// Keyboard and mouse event handling.
//
// The form event handler no longer checks field types directly.
// Instead it calls `form.handle_key(key)` which dispatches to the focused
// FormNode. Each node type handles its own input and returns a FormAction.
// This makes adding new field types zero-boilerplate in events.rs.

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::{
    AppState, ConfirmAction, DashFocus, DeployMsg, DeployState, LogsState,
    NEW_RESOURCE_ITEMS, OverlayLayer, ResourceKind, RunState, Screen, SidebarAction, SidebarItem,
};
use crate::ui::form_node::FormAction;

pub fn handle(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    state.ctrl_hint = key.modifiers.contains(KeyModifiers::CONTROL);

    // Ctrl-C always quits
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        state.should_quit = true;
        return Ok(());
    }

    // F1 toggles help sidebar globally (works on all screens, even with overlay)
    if key.code == KeyCode::F(1) {
        state.help_visible = !state.help_visible;
        return Ok(());
    }

    // Esc closes help sidebar first (priority over screen-specific Esc)
    if key.code == KeyCode::Esc && state.help_visible {
        state.help_visible = false;
        return Ok(());
    }

    // Topmost overlay layer captures all input (Ebene system)
    if state.has_overlay() {
        return handle_overlay(key, state, root);
    }

    match state.screen {
        Screen::Welcome    => handle_welcome(key, state),
        Screen::Dashboard  => handle_dashboard(key, state, root),
        Screen::NewProject => handle_resource_form(key, state, root),
        Screen::TaskWizard => handle_wizard(key, state, root),
    }
}

// ── Overlay layer handler ─────────────────────────────────────────────────────

fn handle_overlay(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    // Peek at the topmost overlay type before potentially popping it
    let overlay_kind = state.top_overlay().map(|o| match o {
        OverlayLayer::Logs(_)          => "logs",
        OverlayLayer::Confirm{..}      => "confirm",
        OverlayLayer::Deploy(_)        => "deploy",
        OverlayLayer::NewResource{..}  => "new_resource",
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
            let (_, yes_action) = state.confirm_overlay().unwrap();
            match key.code {
                KeyCode::Char('j') | KeyCode::Char('J')
                | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    state.pop_overlay();
                    match yes_action {
                        ConfirmAction::DeleteProject => delete_selected_project(state, root)?,
                    }
                }
                _ => { state.pop_overlay(); } // any other key = cancel
            }
        }
        Some("deploy") => {
            // Only closeable once done
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

/// Handle keyboard input for the new-resource selector popup.
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

/// Open the form for the resource type at `item_idx` in `NEW_RESOURCE_ITEMS`.
fn open_new_resource_form(item_idx: usize, state: &mut AppState, root: &Path) {
    let Some(&(_, kind)) = NEW_RESOURCE_ITEMS.get(item_idx) else { return };
    match kind {
        ResourceKind::Project => {
            let queue = crate::task_queue::TaskQueue::new(
                crate::task_queue::TaskKind::NewProject,
                state,
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
    let _ = root; // used by callers for context; form submit uses root separately
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
    // Dispatch to the focused FormNode — it handles its own input and navigation
    let action = if let Some(ref mut form) = state.current_form {
        form.handle_key(key)
    } else {
        FormAction::Unhandled
    };

    match action {
        FormAction::Cancel => {
            state.current_form = None;
            state.screen = if state.projects.is_empty() {
                Screen::Welcome
            } else {
                Screen::Dashboard
            };
        }

        FormAction::LangToggle => state.lang = state.lang.toggle(),

        FormAction::Submit => handle_form_submit(state, root)?,

        FormAction::Consumed => {} // node handled it, nothing to do

        FormAction::Unhandled => {
            // Keys not handled by the focused node: lang toggle, quit
            match key.code {
                KeyCode::Char('l') | KeyCode::Char('L') => state.lang = state.lang.toggle(),
                _ => {}
            }
        }

        // These are resolved inside ResourceForm::handle_key before returning
        FormAction::FocusNext | FormAction::FocusPrev
        | FormAction::TabNext  | FormAction::TabPrev
        | FormAction::ValueChanged => {}

        FormAction::Quit => state.should_quit = true,
    }
    Ok(())
}

fn handle_form_submit(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(ref form) = state.current_form else { return Ok(()); };
    let missing_t = form.tab_missing_count(form.active_tab);

    if missing_t > 0 {
        let msg = format!(
            "{} {}",
            missing_t,
            if missing_t == 1 { "Pflichtfeld fehlt" } else { "Pflichtfelder fehlen" }
        );
        if let Some(ref mut f) = state.current_form { f.error = Some(msg); }
        return Ok(());
    }

    if !form.is_last_tab() {
        if let Some(ref mut f) = state.current_form { f.error = None; f.next_tab(); }
        return Ok(());
    }

    let missing = form.missing_required();
    if !missing.is_empty() {
        let msg = format!("{} Pflichtfeld(er) auf anderen Tabs fehlen", missing.len());
        if let Some(ref mut f) = state.current_form { f.error = Some(msg); }
        return Ok(());
    }

    // All good — dispatch to resource-specific submit
    let kind = state.current_form.as_ref().map(|f| f.kind);
    match kind {
        Some(ResourceKind::Project) => submit_project(state, root)?,
        Some(ResourceKind::Service) => submit_service(state, root)?,
        Some(ResourceKind::Host)    => submit_host(state, root)?,
        Some(ResourceKind::Bot)     => submit_bot(state, root)?,
        None => {}
    }
    Ok(())
}

// ── Task Wizard ───────────────────────────────────────────────────────────────

fn handle_wizard(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let action = if let Some(ref mut queue) = state.task_queue {
        if let Some(task) = queue.tasks.get_mut(queue.active) {
            if let Some(ref mut form) = task.form {
                form.handle_key(key)
            } else {
                FormAction::Unhandled
            }
        } else {
            FormAction::Unhandled
        }
    } else {
        FormAction::Unhandled
    };

    match action {
        FormAction::Cancel => {
            state.task_queue = None;
            state.screen = Screen::Dashboard;
        }

        FormAction::LangToggle => state.lang = state.lang.toggle(),

        FormAction::Submit => handle_wizard_submit(state, root)?,

        FormAction::Consumed => {}

        FormAction::Unhandled => {
            match key.code {
                KeyCode::Esc => {
                    state.task_queue = None;
                    state.screen = Screen::Dashboard;
                }
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

fn handle_wizard_submit(state: &mut AppState, root: &Path) -> Result<()> {
    // Validate: check missing required fields on active tab first
    let (missing_tab, is_last, missing_all, kind) = {
        let Some(ref queue) = state.task_queue else { return Ok(()); };
        let Some(task) = queue.tasks.get(queue.active) else { return Ok(()); };
        let Some(ref form) = task.form else { return Ok(()); };
        (
            form.tab_missing_count(form.active_tab),
            form.is_last_tab(),
            form.missing_required().len(),
            task.kind.resource_kind(),
        )
    };

    if missing_tab > 0 {
        let msg = format!(
            "{} {}",
            missing_tab,
            if missing_tab == 1 { "Pflichtfeld fehlt" } else { "Pflichtfelder fehlen" }
        );
        if let Some(ref mut queue) = state.task_queue {
            if let Some(task) = queue.tasks.get_mut(queue.active) {
                if let Some(ref mut form) = task.form { form.error = Some(msg); }
            }
        }
        return Ok(());
    }

    if !is_last {
        if let Some(ref mut queue) = state.task_queue {
            if let Some(task) = queue.tasks.get_mut(queue.active) {
                if let Some(ref mut form) = task.form { form.error = None; form.next_tab(); }
            }
        }
        return Ok(());
    }

    if missing_all > 0 {
        let msg = format!("{} Pflichtfeld(er) auf anderen Tabs fehlen", missing_all);
        if let Some(ref mut queue) = state.task_queue {
            if let Some(task) = queue.tasks.get_mut(queue.active) {
                if let Some(ref mut form) = task.form { form.error = Some(msg); }
            }
        }
        return Ok(());
    }

    // Extract the form, run it through the normal submit path, then advance the queue
    let form = if let Some(ref mut queue) = state.task_queue {
        if let Some(task) = queue.tasks.get_mut(queue.active) {
            task.form.take()
        } else { None }
    } else { None };

    let Some(form) = form else { return Ok(()); };
    state.current_form = Some(form);

    let submit_result = match kind {
        ResourceKind::Project => submit_project(state, root),
        ResourceKind::Host    => submit_host(state, root),
        ResourceKind::Service => submit_service(state, root),
        ResourceKind::Bot     => submit_bot(state, root),
    };

    // Put form back if submit failed (error displayed)
    if let Some(ref mut queue) = state.task_queue {
        if let Some(task) = queue.tasks.get_mut(queue.active) {
            if task.form.is_none() {
                task.form = state.current_form.take();
            }
        }
    }

    submit_result?;

    // If submit succeeded, current_form is None (cleared by submit_*).
    // Advance the wizard queue (take + put back to avoid borrow conflict).
    let more = if let Some(mut queue) = state.task_queue.take() {
        let has_more = queue.on_task_saved(state);
        state.task_queue = Some(queue);
        has_more
    } else {
        false
    };

    if !more {
        // Wizard complete — return to dashboard
        state.task_queue = None;
        state.screen = Screen::Dashboard;
    } else {
        // Stay on wizard screen, next task is now active
        state.screen = Screen::TaskWizard;
    }
    state.current_form = None;
    Ok(())
}

// ── Dashboard ─────────────────────────────────────────────────────────────────

fn handle_dashboard(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    match state.dash_focus {
        // ── Sidebar ────────────────────────────────────────────────────────
        DashFocus::Sidebar => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => state.should_quit = true,
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

            // 'n' — open the new-resource selector (Project / Host / Service / Bot).
            KeyCode::Char('n') => {
                state.push_overlay(OverlayLayer::NewResource { selected: 0 });
            }

            // Context-aware 'e': edit the item under the cursor (project or host).
            KeyCode::Char('e') => {
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
                    _ => {}
                }
            }

            // Enter activates the current sidebar item.
            KeyCode::Enter => {
                let item = state.current_sidebar_item().cloned();
                match item {
                    Some(SidebarItem::Action { kind: SidebarAction::NewProject, .. }) => {
                        state.current_form = Some(crate::project_form::new_project_form());
                        state.screen = Screen::NewProject;
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
                        if let Some(idx) = state.projects.iter().position(|p| p.slug == slug) {
                            state.selected_project = idx;
                            reload_hosts(state, root);
                            state.rebuild_services();
                        }
                        state.dash_focus = DashFocus::Services;
                    }
                    Some(SidebarItem::Host { .. }) | Some(SidebarItem::Service { .. })
                    | Some(SidebarItem::Action { .. }) => {
                        state.dash_focus = DashFocus::Services;
                    }
                    _ => {}
                }
            }

            // Context-aware 'x': delete project or (future) host.
            KeyCode::Char('x') | KeyCode::Delete => {
                let item = state.current_sidebar_item();
                match item {
                    Some(SidebarItem::Project { .. }) if !state.projects.is_empty() => {
                        state.push_overlay(OverlayLayer::Confirm {
                            message:    "dash.hint.confirm".into(),
                            yes_action: ConfirmAction::DeleteProject,
                        });
                    }
                    _ => {}
                }
            }

            _ => {}
        },

        // ── Services ───────────────────────────────────────────────────────
        DashFocus::Services => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => state.should_quit = true,
            KeyCode::Char('L') => state.lang = state.lang.toggle(),
            KeyCode::Tab => state.dash_focus = DashFocus::Sidebar,

            KeyCode::Up   => { if state.selected > 0 { state.selected -= 1; } }
            KeyCode::Down => {
                if state.selected + 1 < state.services.len() { state.selected += 1; }
            }

            // 'n' — open the new-resource selector (same as sidebar).
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
                    let _ = std::process::Command::new("podman")
                        .args(["stop", &svc.name]).output();
                    let _ = std::process::Command::new("podman")
                        .args(["rm",   &svc.name]).output();
                    state.services.remove(state.selected);
                    if state.selected > 0 && state.selected >= state.services.len() {
                        state.selected -= 1;
                    }
                }
            }

            _ => {}
        },
    }
    Ok(())
}

// ── Deploy (background thread) ────────────────────────────────────────────────
//
// Phase 1: Compose export (distributable templates for Docker/Podman Compose)
// Phase 2: Quadlet generation (systemd units for local Podman deployment)
//
// Both phases run in a single background thread — no async needed
// since all I/O is synchronous (registry scanning, file writes).

/// Spawn a background deploy thread. Generates Compose + Quadlet files and
/// reports each step via the deploy progress overlay.
fn trigger_deploy(
    state:       &mut AppState,
    root:        &Path,
    slug:        String,
    project_cfg: fsn_core::config::ProjectConfig,
    host_cfg:    Option<fsn_core::config::HostConfig>,
) {
    let (tx, rx) = std::sync::mpsc::channel::<DeployMsg>();
    state.deploy_rx = Some(rx);
    state.push_overlay(OverlayLayer::Deploy(DeployState {
        target:  project_cfg.project.name.clone(),
        log:     Vec::new(),
        done:    false,
        success: false,
    }));

    let project_dir = root.join("projects").join(&slug);
    let modules_dir = root.join("modules");

    std::thread::spawn(move || {
        // ── Phase 1: Compose export ───────────────────────────────────────────
        let compose_dir = project_dir.join("compose");
        let _ = tx.send(DeployMsg::Log("── Compose-Export ──".into()));

        if let Err(e) = std::fs::create_dir_all(&compose_dir) {
            let _ = tx.send(DeployMsg::Done { success: false, error: Some(e.to_string()) });
            return;
        }

        let compose_content = fsn_engine::generate::compose::generate_compose(&project_cfg);
        if let Err(e) = std::fs::write(compose_dir.join("compose.yml"), &compose_content) {
            let _ = tx.send(DeployMsg::Done { success: false, error: Some(format!("compose.yml: {e}")) });
            return;
        }
        let _ = tx.send(DeployMsg::Log("✓ compose/compose.yml".into()));

        let env_content = fsn_engine::generate::compose::generate_env_example(&project_cfg);
        if let Err(e) = std::fs::write(compose_dir.join(".env.example"), &env_content) {
            let _ = tx.send(DeployMsg::Done { success: false, error: Some(format!(".env.example: {e}")) });
            return;
        }
        let _ = tx.send(DeployMsg::Log("✓ compose/.env.example".into()));

        // ── Phase 2: Quadlet generation ───────────────────────────────────────
        let _ = tx.send(DeployMsg::Log("── Quadlet-Generierung ──".into()));

        // Load module registry
        let registry = match fsn_core::config::ServiceRegistry::load(&modules_dir) {
            Ok(r)  => r,
            Err(e) => {
                let _ = tx.send(DeployMsg::Log(format!("✗ Registry: {e}")));
                let _ = tx.send(DeployMsg::Done { success: false, error: Some("Registry konnte nicht geladen werden".into()) });
                return;
            }
        };

        // Need a HostConfig for resolve_desired — use provided host or a minimal default
        let host = match host_cfg {
            Some(h) => h,
            None => {
                let _ = tx.send(DeployMsg::Log("! Kein Host konfiguriert — Quadlets übersprungen".into()));
                let _ = tx.send(DeployMsg::Log("  → Bitte zuerst einen Host anlegen (Sidebar → n)".into()));
                let _ = tx.send(DeployMsg::Done { success: true, error: None });
                return;
            }
        };

        // Load vault (empty if not found)
        let vault = fsn_core::config::VaultConfig::load(&project_dir, None)
            .unwrap_or_default();

        // Resolve desired state (cross-service vars, env expansion, sub-services, volumes)
        let data_root = project_dir.join("data");
        let desired = match fsn_engine::resolve::resolve_desired(&project_cfg, &host, &registry, &vault, Some(&data_root)) {
            Ok(d)  => d,
            Err(e) => {
                let _ = tx.send(DeployMsg::Done { success: false, error: Some(format!("Resolve: {e}")) });
                return;
            }
        };

        // Write Quadlet files to project_dir/quadlets/
        let quadlet_dir = project_dir.join("quadlets");
        if let Err(e) = std::fs::create_dir_all(&quadlet_dir) {
            let _ = tx.send(DeployMsg::Done { success: false, error: Some(e.to_string()) });
            return;
        }

        let network_name = format!("fsn-{}", project_cfg.project.name.to_lowercase().replace(' ', "-"));

        // Write network unit
        let net_content = fsn_engine::generate::quadlet::generate_network(&network_name, &project_cfg.project.name);
        if let Err(e) = std::fs::write(quadlet_dir.join(format!("{network_name}.network")), &net_content) {
            let _ = tx.send(DeployMsg::Log(format!("✗ {network_name}.network: {e}")));
        } else {
            let _ = tx.send(DeployMsg::Log(format!("✓ {network_name}.network")));
        }

        // Flatten all instances (sub-services first, then parents)
        let mut all_instances = Vec::new();
        for svc in &desired.services {
            for sub in &svc.sub_services {
                all_instances.push(sub);
            }
            all_instances.push(svc);
        }

        for instance in &all_instances {
            match fsn_engine::generate::quadlet::generate(instance, Some(&network_name)) {
                Ok(content) => {
                    let fname = format!("{}.container", instance.name);
                    if let Err(e) = std::fs::write(quadlet_dir.join(&fname), &content) {
                        let _ = tx.send(DeployMsg::Log(format!("✗ {fname}: {e}")));
                    } else {
                        let _ = tx.send(DeployMsg::Log(format!("✓ {fname}")));
                    }

                    // Write env file
                    let env_fname = format!("{}.env", instance.name);
                    let env_lines: String = instance.resolved_env.iter()
                        .map(|(k, v)| format!("{k}={v}\n"))
                        .collect();
                    if let Err(e) = std::fs::write(quadlet_dir.join(&env_fname), &env_lines) {
                        let _ = tx.send(DeployMsg::Log(format!("✗ {env_fname}: {e}")));
                    } else {
                        let _ = tx.send(DeployMsg::Log(format!("✓ {env_fname}")));
                    }
                }
                Err(e) => {
                    let _ = tx.send(DeployMsg::Log(format!("✗ {}: {e}", instance.name)));
                }
            }
        }

        let _ = tx.send(DeployMsg::Done { success: true, error: None });
    });
}

fn delete_selected_project(state: &mut AppState, root: &Path) -> Result<()> {
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

fn reload_hosts(state: &mut AppState, root: &Path) {
    if let Some(proj) = state.projects.get(state.selected_project) {
        state.hosts = crate::load_hosts(&root.join("projects").join(&proj.slug));
        state.rebuild_sidebar();
    }
}

/// Called after `sidebar_cursor` moves — syncs `selected_project` / `selected_host`
/// and reloads dependent data when a Project item is selected.
fn sync_sidebar_selection(state: &mut AppState, root: &Path) {
    match state.current_sidebar_item().cloned() {
        Some(SidebarItem::Project { slug, .. }) => {
            if let Some(idx) = state.projects.iter().position(|p| p.slug == slug) {
                state.selected_project = idx;
                reload_hosts(state, root);
                state.rebuild_services();
            }
        }
        Some(SidebarItem::Host { slug, .. }) => {
            if let Some(idx) = state.hosts.iter().position(|h| h.slug == slug) {
                state.selected_host = idx;
            }
        }
        _ => {}
    }
}

// ── Form submit dispatch ──────────────────────────────────────────────────────

fn submit_project(state: &mut AppState, root: &Path) -> Result<()> {
    let result = state.current_form.as_ref()
        .map(|form| crate::project_form::submit_project_form(form, root));

    match result {
        Some(Ok(())) => {
            state.projects = crate::load_projects(root);
            if let Some(ref form) = state.current_form {
                let slug = form.edit_id.clone()
                    .unwrap_or_else(|| crate::app::slugify(&form.field_value("name")));
                state.selected_project = state.projects.iter()
                    .position(|p| p.slug == slug).unwrap_or(0);
            }
            state.rebuild_services();
            state.rebuild_sidebar();
            state.screen     = Screen::Dashboard;
            state.dash_focus = DashFocus::Sidebar;
            state.current_form = None;
        }
        Some(Err(e)) => {
            if let Some(ref mut form) = state.current_form {
                form.error = Some(format!("{}", e));
            }
        }
        None => {}
    }
    Ok(())
}

fn submit_service(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project).cloned() else {
        if let Some(ref mut f) = state.current_form {
            f.error = Some("Kein Projekt ausgewählt".into());
        }
        return Ok(());
    };

    let project_dir  = root.join("projects").join(&proj.slug);
    let services_dir = project_dir.join("services");
    std::fs::create_dir_all(&services_dir)?;

    let result = state.current_form.as_ref()
        .map(|form| crate::service_form::submit_service_form(form, &services_dir, &proj.slug));

    match result {
        Some(Ok(())) => {
            // Also register in project.toml [load.services.{slug}]
            if let Some(ref form) = state.current_form {
                let svc_name  = form.field_value("name");
                let svc_class = form.field_value("class");
                let slug      = crate::app::slugify(&svc_name);
                let mut proj_content = std::fs::read_to_string(&proj.toml_path)?;
                if !proj_content.contains(&format!("[load.services.{}]", slug)) {
                    let version  = form.field_value("version");
                    let ver      = if version.is_empty() { "latest".to_string() } else { version };
                    let svc_env  = form.field_value("env");

                    proj_content.push_str(&format!(
                        "\n[load.services.{slug}]\nservice_class = \"{svc_class}\"\nversion       = \"{ver}\"\n"
                    ));

                    // Instance-level env overrides → [load.services.{slug}.env]
                    let env_pairs: Vec<String> = svc_env.lines()
                        .filter_map(|line| {
                            let (k, v) = line.split_once('=')?;
                            let k = k.trim();
                            if k.is_empty() { return None; }
                            Some(format!("{k} = \"{}\"", v.trim()))
                        })
                        .collect();
                    if !env_pairs.is_empty() {
                        proj_content.push_str(&format!(
                            "\n[load.services.{slug}.env]\n{}\n",
                            env_pairs.join("\n")
                        ));
                    }

                    std::fs::write(&proj.toml_path, proj_content)?;
                }
            }
            state.projects = crate::load_projects(root);
            state.rebuild_services();
            state.screen      = Screen::Dashboard;
            state.dash_focus  = DashFocus::Services;
            state.current_form = None;
        }
        Some(Err(e)) => {
            if let Some(ref mut form) = state.current_form {
                form.error = Some(format!("{e}"));
            }
        }
        None => {}
    }
    Ok(())
}

fn submit_host(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project) else {
        if let Some(ref mut f) = state.current_form {
            f.error = Some("Kein Projekt ausgewählt".into());
        }
        return Ok(());
    };
    let project_dir = root.join("projects").join(&proj.slug);

    let result = state.current_form.as_ref()
        .map(|form| crate::host_form::submit_host_form(form, &project_dir));

    match result {
        Some(Ok(())) => {
            state.hosts = crate::load_hosts(&project_dir);
            state.rebuild_sidebar();
            state.screen     = Screen::Dashboard;
            state.dash_focus = DashFocus::Sidebar;
            state.current_form = None;
        }
        Some(Err(e)) => {
            if let Some(ref mut form) = state.current_form {
                form.error = Some(format!("{}", e));
            }
        }
        None => {}
    }
    Ok(())
}

fn submit_bot(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project).cloned() else {
        if let Some(ref mut f) = state.current_form {
            f.error = Some("Kein Projekt ausgewählt".into());
        }
        return Ok(());
    };
    let project_dir = root.join("projects").join(&proj.slug);

    let result = state.current_form.as_ref()
        .map(|form| crate::bot_form::submit_bot_form(form, &project_dir, &proj.slug));

    match result {
        Some(Ok(())) => {
            state.screen      = Screen::Dashboard;
            state.dash_focus  = DashFocus::Services;
            state.current_form = None;
        }
        Some(Err(e)) => {
            if let Some(ref mut form) = state.current_form {
                form.error = Some(format!("{e}"));
            }
        }
        None => {}
    }
    Ok(())
}

// ── Mouse events ──────────────────────────────────────────────────────────────

pub fn handle_mouse(event: MouseEvent, state: &mut AppState) -> Result<()> {
    let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));

    // Overlay scroll support
    match event.kind {
        MouseEventKind::ScrollDown => {
            if let Some(logs) = state.logs_overlay_mut() {
                let max = logs.lines.len().saturating_sub(1);
                if logs.scroll < max { logs.scroll += 1; }
                return Ok(());
            }
        }
        MouseEventKind::ScrollUp => {
            if let Some(logs) = state.logs_overlay_mut() {
                if logs.scroll > 0 { logs.scroll -= 1; }
                return Ok(());
            }
        }
        _ => {}
    }

    match event.kind {
        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
            if state.screen == Screen::NewProject {
                if let Some(ref mut form) = state.current_form {
                    // Find focused SelectInputNode and cycle its options
                    if let Some(idx) = form.focused_node_global_idx() {
                        use crossterm::event::KeyCode;
                        let fake_key = crossterm::event::KeyEvent::new(
                            if matches!(event.kind, MouseEventKind::ScrollDown) {
                                KeyCode::Down
                            } else {
                                KeyCode::Up
                            },
                            KeyModifiers::empty(),
                        );
                        form.nodes[idx].handle_key(fake_key);
                    }
                }
            }
        }

        MouseEventKind::Down(_) => {
            // Effective width: shrink by help sidebar if visible
            let eff_w = if state.help_visible && tw > crate::ui::help_sidebar::SIDEBAR_WIDTH {
                tw - crate::ui::help_sidebar::SIDEBAR_WIDTH
            } else {
                tw
            };

            // Language button — top-right of the main content area
            if event.column >= eff_w.saturating_sub(6) && event.column < eff_w && event.row <= 2 {
                state.lang = state.lang.toggle();
                return Ok(());
            }

            if state.screen == Screen::NewProject {
                handle_form_click(event.column, event.row, state, eff_w);
            } else if state.screen == Screen::Dashboard && !state.has_overlay() {
                handle_dashboard_click(event.column, event.row, state);
            }
        }

        _ => {}
    }
    Ok(())
}

fn handle_form_click(col: u16, row: u16, state: &mut AppState, term_w: u16) {
    let Some(ref mut form) = state.current_form else { return };

    // The form inner area mirrors new_project.rs: 90% centered, header(3)+tabs(3) from top
    let inner_x = term_w * 5 / 100;
    let inner_w = term_w * 90 / 100;
    let inner   = ratatui::layout::Rect { x: inner_x, y: 6, width: inner_w, height: 200 };

    // First: try clicking the focused field's overlay (e.g. dropdown)
    if let Some(global_idx) = form.focused_node_global_idx() {
        if form.nodes[global_idx].click_overlay(col, row, inner) {
            return; // overlay consumed the click
        }
    }

    // Then: focus whichever field was clicked
    form.click_focus(col, row);
}

// ── Dashboard click handler ───────────────────────────────────────────────────

fn handle_dashboard_click(col: u16, row: u16, state: &mut AppState) {
    const SIDEBAR_W: u16 = 22;
    const HEADER_H:  u16 = 3;

    if row < HEADER_H { return; }
    let body_row = row - HEADER_H;

    if col < SIDEBAR_W {
        state.dash_focus = DashFocus::Sidebar;
        // inner area starts 1 row below the block (top padding in render_sidebar)
        const INNER_OFFSET: u16 = 1;
        if body_row < INNER_OFFSET { return; }
        let item_idx = (body_row - INNER_OFFSET) as usize;
        if let Some(item) = state.sidebar_items.get(item_idx) {
            if item.is_selectable() {
                state.sidebar_cursor = item_idx;
                // Note: full sync (reload_hosts, rebuild_services) only via keyboard.
                // Mouse click just moves focus; press a key to activate.
            }
        }
    } else {
        state.dash_focus = DashFocus::Services;
        const TABLE_HEADER: u16 = 1;
        if body_row <= TABLE_HEADER { return; }
        let svc_row = (body_row - TABLE_HEADER - 1) as usize;
        if svc_row < state.services.len() {
            state.selected = svc_row;
        }
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

fn fetch_logs(name: &str) -> Vec<String> {
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
