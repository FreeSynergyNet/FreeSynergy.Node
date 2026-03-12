// Sidebar focus keyboard event handler.
//
// Pattern: Chain of Responsibility — shared shortcuts checked first (via
// handle_dashboard_shared), then sidebar-specific keys.
//
// Sidebar-specific keys: ↑↓ navigation, Enter/e = activate/edit,
// s = start, x/Del = delete confirm, y = yank, / = open filter, S = Settings, T = Store.

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{AppState, DashFocus, Screen, SidebarItem};
use crate::actions::{copy_to_clipboard, sync_sidebar_selection};

use super::actions::activate_sidebar_item;
use super::actions::sidebar_start_resource;
use super::actions::sidebar_confirm_delete;
use super::shortcuts::{handle_dashboard_shared, handle_sidebar_filter_key};

/// Handle keyboard input when dashboard focus is on the sidebar.
pub(super) fn handle_dashboard_sidebar(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
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

        // 'T' = sTore browser (uppercase to avoid conflict with lowercase 's' = start).
        KeyCode::Char('T') => {
            state.store_cursor = 0;
            state.screen = Screen::Store;
        }

        // 'e' = explicit edit (same as Enter on a resource item, but not on Action items).
        KeyCode::Char('e') => {
            if let Some(item) = state.current_sidebar_item().cloned() {
                item.open_edit_form(state, root);
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
