// Mouse event handling — single source of truth for all mouse behavior.
//
// Design Pattern: Single Source of Truth + OOP dispatch via DashHit
//   - Which mouse actions exist:          ContextAction in app.rs
//   - Which actions apply per item type:  SidebarItem::context_actions() in app.rs
//   - How clicks map to UI elements:      dash_hit() — single dispatch for left/right/scroll
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
    ActionSource, AppState, ConfirmAction, ContextAction, DashFocus, LogsState,
    OverlayLayer, RunState, Screen, SidebarItem,
};
use crate::click_map::ClickTarget;
use crate::actions::{fetch_logs, start_service};
use crate::deploy_thread::trigger_deploy;
use crate::events_dashboard::activate_sidebar_item;

// ── Configuration — change here to adjust mouse feel ─────────────────────────

/// Maximum ms between two clicks to count as a double-click.
const DOUBLE_CLICK_MS: u128 = 400;

/// Lines scrolled per scroll-wheel tick.
const SCROLL_STEP: usize = 3;

// ── Dashboard hit result — eliminates triple-duplicated dispatch ──────────────
//
// Pattern: Value Object — represents what the user clicked on in the dashboard.
// handle_left_click, handle_right_click, and handle_scroll all call dash_hit()
// and then branch on the result — no per-function sidebar_hit/services_hit calls.

#[derive(Debug)]
enum DashHit {
    /// A sidebar item at the given items-list index.
    Sidebar(usize),
    /// A service row at the given services-list index.
    Service(usize),
    /// Click did not land in either area.
    Miss,
}

/// Resolve which dashboard element is at (col, row).
/// Single source of truth for hit-testing — called by left-click, right-click, and scroll.
fn dash_hit(col: u16, row: u16, state: &AppState) -> DashHit {
    if let Some(item_idx) = sidebar_hit(col, row, state) {
        return DashHit::Sidebar(item_idx);
    }
    if let Some(svc_idx) = services_hit(col, row, state) {
        return DashHit::Service(svc_idx);
    }
    DashHit::Miss
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn handle_mouse(event: MouseEvent, state: &mut AppState, root: &Path) -> Result<()> {
    // LangToggle and NavTab are global — work on every screen and overlay state.
    // Checked first so they are never blocked by overlays or form popups.
    if event.kind == MouseEventKind::Down(MouseButton::Left) {
        // Clone target to release the immutable borrow on click_map before any
        // mutable state changes (cycle_lang / navigate_to_tab).
        let target = state.click_map.hit(event.column, event.row).cloned();
        match target {
            Some(ClickTarget::LangToggle)      => { state.cycle_lang(); return Ok(()); }
            Some(ClickTarget::NavTab { index }) => { navigate_to_tab(index, state); return Ok(()); }
            _ => {}
        }
    }

    // Never handle mouse while a non-context overlay is open (keyboard takes over).
    if state.has_overlay() {
        return handle_overlay_mouse(event, state);
    }

    // Form screen — delegate entirely to the form handler.
    if state.form_queue.is_some() {
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
        let form = match state.active_form_mut() { Some(f) => f, None => return Ok(()) };
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
    // before mutably borrowing state.form_queue below.
    let target = state.click_map.hit(event.column, event.row).cloned();

    match target {
        Some(ClickTarget::FormField { slot, node_idx, rect }) => {
            if let Some(form) = state.active_form_mut() {
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
        Some(ClickTarget::QueueTab { idx }) => {
            if event.kind == MouseEventKind::Down(MouseButton::Left) {
                if let Some(q) = state.form_queue.as_mut() {
                    q.switch_to(idx);
                }
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

    match dash_hit(col, row, state) {
        DashHit::Sidebar(item_idx) => {
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
        }
        DashHit::Service(svc_idx) if svc_idx < state.services.len() => {
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
        DashHit::Miss => {
            // ClickMap dispatch for non-dashboard screens (e.g. Settings).
            let target = state.click_map.hit(col, row).cloned();
            match target {
                Some(ClickTarget::SettingsSidebar { idx }) => {
                    use crate::app::{SettingsFocus, SettingsSection};
                    state.settings_sidebar_cursor = idx;
                    state.settings_section = SettingsSection::from_idx(idx);
                    if dbl {
                        state.settings_focus = SettingsFocus::Content;
                        state.settings_cursor = 0;
                        state.lang_cursor = 0;
                    } else {
                        state.settings_focus = SettingsFocus::Sidebar;
                    }
                }
                Some(ClickTarget::SettingsCursor { idx }) => {
                    use crate::app::SettingsFocus;
                    state.settings_cursor = idx;
                    state.settings_focus = SettingsFocus::Content;
                    if dbl {
                        // Double-click on store row — open edit form.
                        if let Some(store) = state.settings.stores.get(idx) {
                            let form = crate::settings_form::edit_store_form(idx, store);
                            state.open_form(form);
                        }
                    }
                }
                Some(ClickTarget::LangCursor { idx }) => {
                    use crate::app::SettingsFocus;
                    state.lang_cursor = idx;
                    state.settings_focus = SettingsFocus::Content;
                    if dbl {
                        // Double-click: activate language (Enter behavior).
                        crate::events::lang_cursor_activate_pub(state, idx);
                    } else {
                        // Single-click: toggle checkbox (Space behavior).
                        crate::events::lang_cursor_toggle_pub(state, idx);
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Right click → context menu ────────────────────────────────────────────────

fn handle_right_click(col: u16, row: u16, state: &mut AppState) {
    match dash_hit(col, row, state) {
        DashHit::Sidebar(item_idx) => {
            if let Some(item) = state.sidebar_items.get(item_idx).cloned() {
                let items = item.context_actions();
                if !items.is_empty() {
                    state.sidebar_cursor = item_idx;
                    state.dash_focus = DashFocus::Sidebar;
                    state.push_overlay(OverlayLayer::ContextMenu {
                        x: col, y: row, items, selected: 0,
                        source: Some(ActionSource::Sidebar(item)),
                    });
                }
            }
        }
        DashHit::Service(svc_idx) if svc_idx < state.services.len() => {
            let svc_name = state.services[svc_idx].name.clone();
            state.selected = svc_idx;
            state.dash_focus = DashFocus::Services;

            // Locate the matching sidebar item (always present for current project's services).
            // Reuse item.context_actions() and item.delete_confirm() — Single Source of Truth.
            let sidebar_item = state.sidebar_items.iter()
                .find(|i| matches!(i, SidebarItem::Service { name, .. } if name == &svc_name))
                .cloned();

            let items = sidebar_item.as_ref()
                .map(|i| i.context_actions())
                .unwrap_or_else(|| {
                    // Fallback (should never happen): build actions from raw status.
                    let ss = if state.services[svc_idx].status == RunState::Running {
                        ContextAction::Stop } else { ContextAction::Start };
                    vec![ss, ContextAction::Logs, ContextAction::Edit, ContextAction::Delete]
                });

            if !items.is_empty() {
                state.push_overlay(OverlayLayer::ContextMenu {
                    x: col, y: row, items, selected: 0,
                    source: sidebar_item.map(ActionSource::Sidebar),
                });
            }
        }
        _ => {}
    }
}

// ── Scroll ────────────────────────────────────────────────────────────────────

fn handle_scroll(col: u16, row: u16, state: &mut AppState, dir: i32) {
    match dash_hit(col, row, state) {
        DashHit::Sidebar(_) | DashHit::Miss if is_in_sidebar(col, state) => {
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
        }
        DashHit::Service(_) => {
            if let Some(area) = state.services_table_area {
                if col >= area.x && col < area.right() && row >= area.y && row < area.bottom() {
                    let cur = state.selected as i32 + dir * SCROLL_STEP as i32;
                    let max = state.services.len().saturating_sub(1) as i32;
                    state.selected = cur.clamp(0, max) as usize;
                }
            }
        }
        _ => {
            // Miss — check if in services area by rect (scroll can hit the area even without a
            // specific service row, e.g. scrolling past the end of the list).
            if let Some(area) = state.services_table_area {
                if col >= area.x && col < area.right() && row >= area.y && row < area.bottom() {
                    let cur = state.selected as i32 + dir * SCROLL_STEP as i32;
                    let max = state.services.len().saturating_sub(1) as i32;
                    state.selected = cur.clamp(0, max) as usize;
                }
            }
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

    let (items, selected, source) = match state.top_overlay() {
        Some(OverlayLayer::ContextMenu { items, selected, source, .. }) => {
            (items.clone(), *selected, source.clone())
        }
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
                execute_context_action(state, root, action, &source)?;
            }
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            state.pop_overlay();
        }
        _ => {}
    }
    Ok(())
}

// ── Context action execution ──────────────────────────────────────────────────
//
// Single dispatch point — edit only here to change what each action does.
//
// `source` carries the item that was right-clicked (set at click-time in
// handle_right_click).  Using source instead of current_sidebar_item() ensures
// that Edit/Delete work correctly even when the click was in the services table
// and the sidebar cursor points to a different item.

/// Execute a context menu action.
///
/// `source`: the item that triggered the menu — `None` for generic menus (e.g. 'n' key).
/// Add new actions here; add new item types to `SidebarItem::context_actions()` in app.rs.
pub fn execute_context_action(
    state: &mut AppState,
    root:  &Path,
    action: ContextAction,
    source: &Option<ActionSource>,
) -> Result<()> {
    // Helper: resolve the source item, falling back to sidebar cursor if no source stored.
    let source_item = |state: &AppState| -> Option<SidebarItem> {
        match source {
            Some(ActionSource::Sidebar(item)) => Some(item.clone()),
            None => state.current_sidebar_item().cloned(),
        }
    };

    match action {
        ContextAction::Edit => {
            if let Some(item) = source_item(state) {
                item.open_edit_form(state);
            }
        }
        ContextAction::Delete => {
            if let Some(item) = source_item(state) {
                if let Some((message, data, yes_action)) = item.delete_confirm() {
                    state.push_overlay(OverlayLayer::Confirm { message, data, yes_action });
                }
            }
        }
        ContextAction::Deploy => {
            if let Some(proj) = state.projects.get(state.selected_project).cloned() {
                let host = state.hosts.first().map(|h| h.config.clone());
                trigger_deploy(state, root, proj, host);
            }
        }
        ContextAction::Start => {
            if let Some(name) = service_name_from_source(source, state) {
                start_service(state, &name);
            }
        }
        ContextAction::Stop => {
            let name = service_name_from_source(source, state);
            if let Some(name) = name {
                state.push_overlay(OverlayLayer::Confirm {
                    message:    "confirm.stop.service".into(),
                    data:       Some(name),
                    yes_action: ConfirmAction::StopService,
                });
            }
        }
        ContextAction::Logs => {
            let name = service_name_from_source(source, state);
            if let Some(name) = name {
                let lines = fetch_logs(&name);
                state.push_overlay(OverlayLayer::Logs(LogsState {
                    service_name: name, lines, scroll: 0,
                }));
            }
        }
        ContextAction::AddService => {
            state.open_form(crate::service_form::new_service_form());
        }
        ContextAction::AddHost => {
            let slugs   = state.projects.iter().map(|p| p.slug.clone()).collect::<Vec<_>>();
            let current = state.projects.get(state.selected_project)
                .map(|p| p.slug.as_str()).unwrap_or("").to_string();
            state.open_form(crate::host_form::new_host_form(slugs, &current));
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

/// Navigate to the screen / focus implied by a header tab click.
///
/// Tab indices:  0=Projects  1=Hosts  2=Services  3=Store  4=Settings
/// Tabs 0-2 all map to Dashboard (the sidebar focus is set by the sidebar cursor,
/// not the tab).  Tab 3 (Store) is not yet implemented.  Tab 4 = Settings.
fn navigate_to_tab(index: usize, state: &mut AppState) {
    match index {
        0..=2 => { state.screen = Screen::Dashboard; }
        4 => {
            state.settings_cursor = 0;
            state.screen = Screen::Settings;
        }
        _ => {}
    }
}

fn is_double_click(col: u16, row: u16, state: &AppState) -> bool {
    state.last_click
        .map(|(lc, lr, lt)| lc == col && lr == row && lt.elapsed() < Duration::from_millis(DOUBLE_CLICK_MS as u64))
        .unwrap_or(false)
}

/// Resolve the service name from the action source, falling back to focus state.
///
/// Source is preferred — it was captured at click-time and reflects the actual
/// item the user interacted with (sidebar or services table).
fn service_name_from_source(source: &Option<ActionSource>, state: &AppState) -> Option<String> {
    match source {
        Some(ActionSource::Sidebar(SidebarItem::Service { name, .. })) => Some(name.clone()),
        _ => {
            // Fallback: sidebar cursor or services table selection.
            match state.current_sidebar_item() {
                Some(SidebarItem::Service { name, .. }) => Some(name.clone()),
                _ => state.services.get(state.selected).map(|s| s.name.clone()),
            }
        }
    }
}
