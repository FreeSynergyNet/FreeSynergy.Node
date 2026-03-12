// Dashboard keyboard event handling — module entry point.
//
// Design Pattern: Chain of Responsibility — key events bubble through:
//   1. handle_dashboard_shared (shortcuts.rs) — q/Esc/n consumed here
//   2. Focus-specific handler  (sidebar.rs or services.rs)
//
// Sub-modules:
//   actions.rs   — activate_sidebar_item, SidebarItem::open_edit_form,
//                  ConfirmAction::execute, handle_new_resource_overlay,
//                  sidebar_start_resource, sidebar_confirm_delete
//   services.rs  — handle_dashboard_services (services-panel key handler)
//   shortcuts.rs — handle_dashboard_shared, handle_sidebar_filter_key
//   sidebar.rs   — handle_dashboard_sidebar (sidebar-panel key handler)
//
// Public API consumed by callers outside this module:
//   handle_dashboard()            — events.rs Screen::Dashboard arm
//   handle_new_resource_overlay() — events.rs OverlayKind::NewResource arm
//   activate_sidebar_item()       — mouse.rs double-click / left-click

pub(crate) mod actions;
mod services;
mod shortcuts;
mod sidebar;

use std::path::Path;

use anyhow::Result;
use crossterm::event::KeyEvent;

use crate::app::{AppState, DashFocus};

// Re-export the symbols that callers outside this module need.
pub use actions::{activate_sidebar_item, handle_new_resource_overlay};

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn handle_dashboard(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    match state.dash_focus {
        DashFocus::Sidebar  => sidebar::handle_dashboard_sidebar(key, state, root),
        DashFocus::Services => services::handle_dashboard_services(key, state, root),
    }
}
