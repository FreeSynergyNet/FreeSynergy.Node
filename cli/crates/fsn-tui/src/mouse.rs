// Mouse event handling.
//
// Scroll support for overlays, form field clicks, dashboard sidebar/table clicks.

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};

use crate::app::{AppState, DashFocus, Screen, SidebarAction, SidebarItem};
use crate::actions::reload_hosts;

pub fn handle_mouse(event: MouseEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));

    // Overlay scroll support (logs panel)
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
                handle_dashboard_click(event.column, event.row, state, root);
            }
        }

        _ => {}
    }
    Ok(())
}

fn handle_form_click(col: u16, row: u16, state: &mut AppState, term_w: u16) {
    let Some(ref mut form) = state.current_form else { return };

    let inner_x = term_w * 5 / 100;
    let inner_w = term_w * 90 / 100;
    let inner   = ratatui::layout::Rect { x: inner_x, y: 6, width: inner_w, height: 200 };

    if let Some(global_idx) = form.focused_node_global_idx() {
        if form.nodes[global_idx].click_overlay(col, row, inner) {
            return;
        }
    }

    form.click_focus(col, row);
}

fn handle_dashboard_click(col: u16, row: u16, state: &mut AppState, root: &Path) {
    const SIDEBAR_W: u16 = 28;
    const HEADER_H:  u16 = 3;

    if row < HEADER_H { return; }
    let body_row = row - HEADER_H;

    if col < SIDEBAR_W {
        state.dash_focus = DashFocus::Sidebar;
        const INNER_OFFSET: u16 = 1;
        if body_row < INNER_OFFSET { return; }
        let item_idx = (body_row - INNER_OFFSET) as usize;
        if let Some(item) = state.sidebar_items.get(item_idx).cloned() {
            if item.is_selectable() {
                state.sidebar_cursor = item_idx;
                match &item {
                    SidebarItem::Action { kind: SidebarAction::NewProject, .. } => {
                        let queue = crate::task_queue::TaskQueue::new(
                            crate::task_queue::TaskKind::NewProject, state,
                        );
                        state.task_queue = Some(queue);
                        state.screen = crate::app::Screen::TaskWizard;
                    }
                    SidebarItem::Action { kind: SidebarAction::NewHost, .. } => {
                        let project_slugs = state.projects.iter().map(|p| p.slug.clone()).collect();
                        let current = state.projects.get(state.selected_project)
                            .map(|p| p.slug.as_str()).unwrap_or("").to_string();
                        state.current_form = Some(crate::host_form::new_host_form(project_slugs, &current));
                        state.screen = crate::app::Screen::NewProject;
                    }
                    SidebarItem::Action { kind: SidebarAction::NewService, .. } => {
                        state.current_form = Some(crate::service_form::new_service_form());
                        state.screen = crate::app::Screen::NewProject;
                    }
                    SidebarItem::Project { slug, .. } => {
                        if let Some(idx) = state.projects.iter().position(|p| p.slug == *slug) {
                            state.selected_project = idx;
                            reload_hosts(state, root);
                            state.rebuild_services();
                        }
                    }
                    _ => {}
                }
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
