// Mouse event handling — single source of truth for all mouse behavior.
//
// Design Pattern: Single Source of Truth
//   - Which mouse actions exist:          ContextAction in app.rs
//   - Which actions apply per item type:  context_items_for()  ← edit here
//   - How clicks map to UI elements:      sidebar_hit(), services_hit()
//   - How actions are executed:           execute_context_action() ← edit here
//
// Called from events.rs → fsn_event → Event::Mouse branch.
// Reuses keyboard action helpers from events_dashboard (pub(crate) fns).
//
// Double-click threshold: DOUBLE_CLICK_MS.
// Scroll behaviour: SCROLL_STEP lines per scroll event.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::app::{
    AppState, ConfirmAction, ContextAction, DashFocus, LogsState, NotifKind,
    OverlayLayer, RunState, SidebarItem,
};
use crate::click_map::ClickTarget;
use crate::actions::{fetch_logs, podman_status};
use crate::deploy_thread::trigger_deploy;
use crate::events_dashboard::activate_sidebar_item;

// ── Configuration — change here to adjust mouse feel ─────────────────────────

/// Maximum ms between two clicks to count as a double-click.
const DOUBLE_CLICK_MS: u128 = 400;

/// Lines scrolled per scroll-wheel tick.
const SCROLL_STEP: usize = 3;

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn handle_mouse(event: MouseEvent, state: &mut AppState, root: &Path) -> Result<()> {
    // LangToggle is global — works on every screen/overlay state.
    // Checked first so it is never blocked by overlays or form popups.
    if event.kind == MouseEventKind::Down(MouseButton::Left) {
        if let Some(ClickTarget::LangToggle) = state.click_map.hit(event.column, event.row) {
            state.lang = state.lang.toggle();
            return Ok(());
        }
    }

    // Never handle mouse while a non-context overlay is open (keyboard takes over).
    if state.has_overlay() {
        return handle_overlay_mouse(event, state);
    }

    // Form screen — delegate entirely to the form handler.
    if state.current_form.is_some() {
        return handle_mouse_form(event, state, root);
    }

    match event.kind {
        MouseEventKind::Down(MouseButton::Left)  => handle_left_click(event.column, event.row, state, root)?,
        MouseEventKind::Down(MouseButton::Right) => handle_right_click(event.column, event.row, state),
        MouseEventKind::ScrollUp                 => handle_scroll(event.column, event.row, state, -1),
        MouseEventKind::ScrollDown               => handle_scroll(event.column, event.row, state,  1),
        _ => {}
    }
    Ok(())
}

// ── Scroll inside overlays (e.g. Logs) ───────────────────────────────────────

fn handle_overlay_mouse(event: MouseEvent, state: &mut AppState) -> Result<()> {
    match event.kind {
        MouseEventKind::ScrollUp => {
            if let Some(logs) = state.logs_overlay_mut() {
                if logs.scroll > 0 { logs.scroll = logs.scroll.saturating_sub(SCROLL_STEP); }
            }
        }
        MouseEventKind::ScrollDown => {
            if let Some(logs) = state.logs_overlay_mut() {
                let max = logs.lines.len().saturating_sub(1);
                logs.scroll = (logs.scroll + SCROLL_STEP).min(max);
            }
        }
        // Close context menu on any click outside it.
        MouseEventKind::Down(_) => {
            if state.top_overlay().map(|o| o.kind()) == Some(crate::app::OverlayKind::ContextMenu) {
                state.pop_overlay();
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Form mouse handling ───────────────────────────────────────────────────────
//
// Called when `state.current_form.is_some()` and no overlay is open.
// Strategy:
//   1. Popup layer — any open popup intercepts all events.
//   2. ClickMap dispatch — hit-test the registry built during the last render.
//      FormField → focus + delegate to node.handle_mouse().
//      FormSubmit → call form submit handler.
//
// LangToggle is handled before this function is reached (in handle_mouse).

fn handle_mouse_form(event: MouseEvent, state: &mut AppState, root: &Path) -> Result<()> {
    use crate::ui::form_node::FormAction;

    // ── Open-popup layer — intercept all events while any popup is visible ─
    //
    // Popup is its own layer: scroll and clicks go to the popup first.
    // Single-select: click-outside = cancel.  Multi-select: click-outside = accept.
    {
        let form = match state.current_form.as_mut() { Some(f) => f, None => return Ok(()) };
        let popup_idx = form.nodes.iter().position(|n| n.has_open_popup());
        if let Some(idx) = popup_idx {
            let action = form.nodes[idx].handle_popup_mouse(event);
            if matches!(action, Some(FormAction::ValueChanged) | Some(FormAction::AcceptAndNext)) {
                let key = form.nodes[idx].key();
                (form.on_change)(&mut form.nodes, key);
            }
            return Ok(());
        }
    }

    // ── ClickMap dispatch ─────────────────────────────────────────────────
    //
    // Clone the target so we drop the immutable borrow on state.click_map
    // before mutably borrowing state.current_form below.
    let target = state.click_map.hit(event.column, event.row).cloned();

    match target {
        Some(ClickTarget::FormField { slot, node_idx, rect }) => {
            if let Some(form) = state.current_form.as_mut() {
                form.active_field = slot;
                form.touched = true;
                let action = form.nodes[node_idx].handle_mouse(event, rect);
                if action == FormAction::ValueChanged {
                    let key = form.nodes[node_idx].key();
                    (form.on_change)(&mut form.nodes, key);
                }
            }
        }
        Some(ClickTarget::FormSubmit) => {
            if event.kind == MouseEventKind::Down(MouseButton::Left) {
                crate::submit::handle_form_submit(state, root)?;
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Left click ────────────────────────────────────────────────────────────────

fn handle_left_click(col: u16, row: u16, state: &mut AppState, root: &Path) -> Result<()> {
    let dbl = is_double_click(col, row, state);
    state.last_click = Some((col, row, Instant::now()));

    // Sidebar click
    if let Some(item_idx) = sidebar_hit(col, row, state) {
        if state.sidebar_items[item_idx].is_selectable() {
            state.dash_focus = DashFocus::Sidebar;
            state.sidebar_cursor = item_idx;
            crate::actions::sync_sidebar_selection(state, root);

            if dbl {
                // Double-click = activate (same as Enter)
                if let Some(item) = state.current_sidebar_item().cloned() {
                    activate_sidebar_item(item, state, root);
                }
            }
        }
        return Ok(());
    }

    // Services table click
    if let Some(svc_idx) = services_hit(col, row, state) {
        if svc_idx < state.services.len() {
            state.dash_focus = DashFocus::Services;
            state.selected = svc_idx;

            if dbl {
                // Double-click = open logs
                if let Some(svc) = state.services.get(svc_idx) {
                    let lines = fetch_logs(&svc.name);
                    state.push_overlay(OverlayLayer::Logs(LogsState {
                        service_name: svc.name.clone(), lines, scroll: 0,
                    }));
                }
            }
        }
        return Ok(());
    }

    Ok(())
}

// ── Right click → context menu ────────────────────────────────────────────────

fn handle_right_click(col: u16, row: u16, state: &mut AppState) {
    // Try sidebar item first
    if let Some(item_idx) = sidebar_hit(col, row, state) {
        if let Some(item) = state.sidebar_items.get(item_idx) {
            let items = context_items_for(item);
            if !items.is_empty() {
                state.sidebar_cursor = item_idx;
                state.dash_focus = DashFocus::Sidebar;
                state.push_overlay(OverlayLayer::ContextMenu {
                    x: col, y: row, items, selected: 0,
                });
            }
        }
        return;
    }

    // Try services table
    if let Some(svc_idx) = services_hit(col, row, state) {
        if svc_idx < state.services.len() {
            let status = state.services[svc_idx].status;
            state.selected = svc_idx;
            state.dash_focus = DashFocus::Services;
            let items = context_items_for_service(status);
            state.push_overlay(OverlayLayer::ContextMenu {
                x: col, y: row, items, selected: 0,
            });
        }
    }
}

// ── Scroll ────────────────────────────────────────────────────────────────────

fn handle_scroll(col: u16, row: u16, state: &mut AppState, dir: i32) {
    // Sidebar area
    if is_in_sidebar(col, state) {
        let cur = state.sidebar_cursor as i32 + dir * SCROLL_STEP as i32;
        let len = state.sidebar_items.len() as i32;
        let clamped = cur.clamp(0, len - 1) as usize;
        // Jump to nearest selectable item
        let target = if dir > 0 {
            (clamped..state.sidebar_items.len()).find(|&i| state.sidebar_items[i].is_selectable())
        } else {
            (0..=clamped).rev().find(|&i| state.sidebar_items[i].is_selectable())
        };
        if let Some(idx) = target {
            state.sidebar_cursor = idx;
        }
        return;
    }

    // Services area
    if let Some(area) = state.services_table_area {
        if col >= area.x && col < area.right() && row >= area.y && row < area.bottom() {
            let cur = state.selected as i32 + dir * SCROLL_STEP as i32;
            let max = state.services.len().saturating_sub(1) as i32;
            state.selected = cur.clamp(0, max) as usize;
        }
    }
}

// ── Context menu keyboard handling — called from events.rs ───────────────────

pub fn handle_context_menu_key(
    key: crossterm::event::KeyEvent,
    state: &mut AppState,
    root: &Path,
) -> Result<()> {
    use crossterm::event::KeyCode;

    let (items, selected) = match state.top_overlay() {
        Some(OverlayLayer::ContextMenu { items, selected, .. }) => (items.clone(), *selected),
        _ => return Ok(()),
    };

    match key.code {
        KeyCode::Up => {
            if let Some(OverlayLayer::ContextMenu { selected, .. }) = state.top_overlay_mut() {
                if *selected > 0 { *selected -= 1; }
            }
        }
        KeyCode::Down => {
            if let Some(OverlayLayer::ContextMenu { selected, .. }) = state.top_overlay_mut() {
                if *selected + 1 < items.len() { *selected += 1; }
            }
        }
        KeyCode::Enter => {
            state.pop_overlay();
            if let Some(&action) = items.get(selected) {
                execute_context_action(state, root, action)?;
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            state.pop_overlay();
        }
        _ => {}
    }
    Ok(())
}

// ── Context menu item lists — change here to adjust menus per item type ───────

/// Actions available when right-clicking a sidebar item.
/// To add/remove actions per type: edit only this function.
fn context_items_for(item: &SidebarItem) -> Vec<ContextAction> {
    match item {
        SidebarItem::Project { .. } => vec![
            ContextAction::Edit,
            ContextAction::AddService,
            ContextAction::AddHost,
            ContextAction::Deploy,
            ContextAction::Delete,
        ],
        SidebarItem::Host { .. } => vec![
            ContextAction::Edit,
            ContextAction::Deploy,
            ContextAction::Delete,
        ],
        SidebarItem::Service { status, .. } => context_items_for_service(*status),
        _ => vec![],
    }
}

/// Actions for a service row based on its current RunState.
fn context_items_for_service(status: RunState) -> Vec<ContextAction> {
    let start_stop = if status == RunState::Running {
        ContextAction::Stop
    } else {
        ContextAction::Start
    };
    vec![start_stop, ContextAction::Logs, ContextAction::Edit, ContextAction::Delete]
}

// ── Context action execution — change here to adjust what each action does ────

/// Execute a context menu action. Single dispatch point — edit only here.
pub fn execute_context_action(state: &mut AppState, root: &Path, action: ContextAction) -> Result<()> {
    match action {
        ContextAction::Edit => {
            if let Some(item) = state.current_sidebar_item().cloned() {
                crate::events_dashboard::open_edit_form_for_item_pub(&item, state);
            }
        }
        ContextAction::Delete => {
            match state.current_sidebar_item().cloned() {
                Some(SidebarItem::Project { .. }) => {
                    state.push_overlay(OverlayLayer::Confirm {
                        message: "confirm.delete.project".into(), data: None,
                        yes_action: ConfirmAction::DeleteProject,
                    });
                }
                Some(SidebarItem::Host { slug, .. }) => {
                    state.push_overlay(OverlayLayer::Confirm {
                        message: "confirm.delete.host".into(), data: Some(slug),
                        yes_action: ConfirmAction::DeleteHost,
                    });
                }
                Some(SidebarItem::Service { name, .. }) => {
                    state.push_overlay(OverlayLayer::Confirm {
                        message: "confirm.delete.service".into(), data: Some(name),
                        yes_action: ConfirmAction::DeleteService,
                    });
                }
                _ => {}
            }
        }
        ContextAction::Deploy => {
            if let Some(proj) = state.projects.get(state.selected_project).cloned() {
                let host = state.hosts.first().map(|h| h.config.clone());
                trigger_deploy(state, root, proj, host);
            }
        }
        ContextAction::Start => {
            // Works for both sidebar service and services-table selection
            let name = service_name_at_cursor(state);
            if let Some(name) = name {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "start", &format!("{}.service", name)])
                    .output();
                if let Some(row) = state.services.iter_mut().find(|s| s.name == name) {
                    row.status = podman_status(&name);
                }
                state.push_notif(NotifKind::Info, format!("Service '{}' gestartet", name));
            }
        }
        ContextAction::Stop => {
            let name = service_name_at_cursor(state);
            if let Some(name) = name {
                state.push_overlay(OverlayLayer::Confirm {
                    message:    "confirm.stop.service".into(),
                    data:       Some(name),
                    yes_action: ConfirmAction::StopService,
                });
            }
        }
        ContextAction::Logs => {
            let name = service_name_at_cursor(state);
            if let Some(name) = name {
                let lines = fetch_logs(&name);
                state.push_overlay(OverlayLayer::Logs(LogsState {
                    service_name: name, lines, scroll: 0,
                }));
            }
        }
        ContextAction::AddService => {
            state.current_form = Some(crate::service_form::new_service_form());
            state.screen = crate::app::Screen::NewProject;
        }
        ContextAction::AddHost => {
            let slugs   = state.projects.iter().map(|p| p.slug.clone()).collect::<Vec<_>>();
            let current = state.projects.get(state.selected_project)
                .map(|p| p.slug.as_str()).unwrap_or("").to_string();
            state.current_form = Some(crate::host_form::new_host_form(slugs, &current));
            state.screen = crate::app::Screen::NewProject;
        }
    }
    Ok(())
}

// ── Hit-testing helpers ───────────────────────────────────────────────────────

/// Returns the `sidebar_items` index for a click at (col, row), or None.
fn sidebar_hit(col: u16, row: u16, state: &AppState) -> Option<usize> {
    let area = state.sidebar_list_area?;
    if col < area.x || col >= area.right() { return None; }
    if row < area.y || row >= area.bottom() { return None; }
    let rel = (row - area.y) as usize;
    let visible = state.visible_sidebar_items();
    visible.get(rel).map(|(idx, _)| *idx)
}

/// Returns the service index for a click in the services table, or None.
/// Row 0 = header (ignored).
fn services_hit(col: u16, row: u16, state: &AppState) -> Option<usize> {
    let area = state.services_table_area?;
    if col < area.x || col >= area.right() { return None; }
    if row < area.y || row >= area.bottom() { return None; }
    let rel = (row - area.y) as usize;
    if rel == 0 { return None; } // header row
    let svc_idx = rel.saturating_sub(1);
    if svc_idx < state.services.len() { Some(svc_idx) } else { None }
}

fn is_in_sidebar(col: u16, state: &AppState) -> bool {
    state.sidebar_list_area
        .map(|a| col >= a.x && col < a.right())
        .unwrap_or(false)
}

fn is_double_click(col: u16, row: u16, state: &AppState) -> bool {
    state.last_click
        .map(|(lc, lr, lt)| lc == col && lr == row && lt.elapsed() < Duration::from_millis(DOUBLE_CLICK_MS as u64))
        .unwrap_or(false)
}

fn service_name_at_cursor(state: &AppState) -> Option<String> {
    // Prefer sidebar service item, fall back to services table selection
    match state.current_sidebar_item() {
        Some(SidebarItem::Service { name, .. }) => Some(name.clone()),
        _ => state.services.get(state.selected).map(|s| s.name.clone()),
    }
}
