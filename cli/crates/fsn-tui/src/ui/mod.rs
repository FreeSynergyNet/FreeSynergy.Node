// UI rendering — dispatches to screen-specific renderers.

pub mod dashboard;
pub mod logs;
pub mod new_project;
pub mod welcome;
pub mod widgets;

use ratatui::Frame;
use crate::app::{AppState, Screen};

pub fn render(f: &mut Frame, state: &AppState) {
    match state.screen {
        Screen::Welcome    => welcome::render(f, state),
        Screen::Dashboard  => dashboard::render(f, state),
        Screen::NewProject => new_project::render(f, state),
    }

    // Overlays drawn on top
    if state.logs_overlay.is_some() {
        logs::render(f, state);
    }
}
