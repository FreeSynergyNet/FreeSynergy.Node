// Mouse event handling.
//
// Scroll support for overlays, form field clicks, dashboard sidebar/table clicks.
// Dashboard sidebar click handling delegates to `events::activate_sidebar_item()`
// so that keyboard and mouse produce identical behavior from a single code path.

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::{AppState, DashFocus, Screen};
use crate::actions::reload_hosts;
use crate::events::activate_sidebar_item;

pub fn handle_mouse(event: MouseEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));

    // Overlay scroll: logs panel scrolls with mouse wheel.
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
        // Form scroll: forward wheel events to the focused field as Up/Down keys.
        // This lets SelectInputNode cycle options without keyboard.
        MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
            if state.screen == Screen::NewProject {
                if let Some(ref mut form) = state.current_form {
                    if let Some(idx) = form.focused_node_global_idx() {
                        use crossterm::event::KeyCode;
                        let fake_key = crossterm::event::KeyEvent::new(
                            if matches!(event.kind, MouseEventKind::ScrollDown) { KeyCode::Down } else { KeyCode::Up },
                            KeyModifiers::empty(),
                        );
                        form.nodes[idx].handle_key(fake_key);
                    }
                }
            }
        }

        MouseEventKind::Down(_) => {
            // Shrink effective width when the help sidebar is visible.
            let eff_w = if state.help_visible && tw > crate::ui::help_sidebar::SIDEBAR_WIDTH {
                tw - crate::ui::help_sidebar::SIDEBAR_WIDTH
            } else {
                tw
            };

            // Language button — top-right corner of the main content area.
            if event.column >= eff_w.saturating_sub(6) && event.column < eff_w && event.row <= 2 {
                state.lang = state.lang.toggle();
                return Ok(());
            }

            if state.screen == Screen::NewProject {
                handle_form_click(event.column, event.row, state, eff_w);
            } else if state.screen == Screen::Dashboard && !state.has_overlay() {
                handle_dashboard_click(event.column, event.row, state, root);
            }
        }

        _ => {}
    }
    Ok(())
}

fn handle_form_click(col: u16, row: u16, state: &mut AppState, term_w: u16) {
    let Some(ref mut form) = state.current_form else { return };

    // Form inner area: 5% margin on each side (90% width), header(3)+tabs(3) = y:6.
    // height:200 is a sentinel — larger than any realistic terminal.
    let inner_x = term_w * 5 / 100;
    let inner_w = term_w * 90 / 100;
    let inner   = ratatui::layout::Rect { x: inner_x, y: 6, width: inner_w, height: 200 };

    // Let the focused field handle the click first (e.g. close a dropdown).
    if let Some(global_idx) = form.focused_node_global_idx() {
        if form.nodes[global_idx].click_overlay(col, row, inner) {
            return;
        }
    }

    form.click_focus(col, row);
}

fn handle_dashboard_click(col: u16, row: u16, state: &mut AppState, root: &Path) {
    // These constants must match the layout produced by ui/dashboard.rs.
    const SIDEBAR_W: u16 = 28; // width of the left sidebar block
    const HEADER_H:  u16 = 3;  // rows consumed by the header

    if row < HEADER_H { return; }
    let body_row = row - HEADER_H;

    if col < SIDEBAR_W {
        // ── Sidebar click ──────────────────────────────────────────────────
        state.dash_focus = DashFocus::Sidebar;
        // Sidebar block has 1 row of padding at top before the first item.
        const INNER_OFFSET: u16 = 1;
        if body_row < INNER_OFFSET { return; }
        let item_idx = (body_row - INNER_OFFSET) as usize;
        if let Some(item) = state.sidebar_items.get(item_idx).cloned() {
            if item.is_selectable() {
                state.sidebar_cursor = item_idx;
                // Project clicks also reload dependent data (hosts, services).
                if let crate::app::SidebarItem::Project { ref slug, .. } = item {
                    if let Some(idx) = state.projects.iter().position(|p| p.slug == *slug) {
                        state.selected_project = idx;
                        reload_hosts(state, root);
                        state.rebuild_services();
                    }
                }
                // Use the same activation logic as keyboard Enter.
                activate_sidebar_item(item, state, root);
            }
        }
    } else {
        // ── Services table click ───────────────────────────────────────────
        state.dash_focus = DashFocus::Services;
        // Table has a 1-row header, then data rows start at offset 2 from body.
        const TABLE_HEADER: u16 = 1;
        if body_row <= TABLE_HEADER { return; }
        let svc_row = (body_row - TABLE_HEADER - 1) as usize;
        if svc_row < state.services.len() {
            state.selected = svc_row;
        }
    }
}
