// Reusable widget helpers.
//
// Design Pattern: Utility Library — stateless pure functions shared across all
// rendering modules. Keeps rendering code DRY: color, char, and text helpers
// live here so no module duplicates a RunState color match or a truncation loop.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear},
    Frame,
};

use crate::app::{run_state_i18n, AppState, RunState};

/// Language toggle button: "[DE]" or "[EN]" in the top-right corner.
pub fn lang_button<'a>(state: &AppState) -> Span<'a> {
    lang_button_raw(state.lang)
}

/// Language toggle button without requiring a full AppState reference.
/// Used by form screens that only have `lang: Lang`.
pub fn lang_button_raw(lang: crate::app::Lang) -> Span<'static> {
    Span::styled(
        format!("[{}]", lang.label()),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
}

/// Status badge with color, backed by fsn-core's `RunState`.
pub fn status_span(status: RunState, state: &AppState) -> Span<'static> {
    let (label, color) = match status {
        RunState::Running => (state.t(run_state_i18n(RunState::Running)), Color::Green),
        RunState::Stopped => (state.t(run_state_i18n(RunState::Stopped)), Color::Yellow),
        RunState::Failed  => (state.t(run_state_i18n(RunState::Failed)),  Color::Red),
        RunState::Missing => (state.t(run_state_i18n(RunState::Missing)), Color::Gray),
    };
    Span::styled(label, Style::default().fg(color))
}

/// Centered popup box — clears background and draws a bordered block.
pub fn popup_area(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    use ratatui::layout::{Constraint, Direction, Layout};
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1]);
    horiz[1]
}

/// Draw a clear + bordered block at `area` (used for overlays).
pub fn clear_block(f: &mut Frame, area: Rect, title: &str) {
    f.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!("─ {} ", title))
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(block, area);
}

/// TUI color for a RunState — single source of truth, avoids duplicated match blocks.
pub fn run_state_color(state: RunState) -> Color {
    match state {
        RunState::Running => Color::Green,
        RunState::Stopped => Color::DarkGray,
        RunState::Failed  => Color::Red,
        RunState::Missing => Color::DarkGray,
    }
}

/// Status character for a RunState — single source of truth.
pub fn run_state_char(state: RunState) -> &'static str {
    match state {
        RunState::Running => "●",
        RunState::Stopped => "○",
        RunState::Failed  => "✕",
        RunState::Missing => "·",
    }
}

/// Truncate a "prefix + name" string to fit within `max_w` characters.
/// Appends "…" when the text must be cut. Used by sidebar and detail renderers.
pub fn truncate(prefix: &str, name: &str, max_w: usize) -> String {
    let total = prefix.len() + name.len();
    if total > max_w && max_w > prefix.len() + 1 {
        format!("{}{}…", prefix, &name[..max_w - prefix.len() - 1])
    } else {
        format!("{}{}", prefix, name)
    }
}

/// Button widget — filled if focused, bordered if not.
pub fn button_line(label: &str, focused: bool, disabled: bool) -> Line<'static> {

    let (fg, bg, modifier) = if disabled {
        (Color::DarkGray, Color::Reset, Modifier::empty())
    } else if focused {
        (Color::Black, Color::Cyan, Modifier::BOLD)
    } else {
        (Color::Cyan, Color::Reset, Modifier::empty())
    };
    Line::from(Span::styled(
        format!("  {}  ", label),
        Style::default().fg(fg).bg(bg).add_modifier(modifier),
    ))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::RunState;

    #[test]
    fn truncate_fits_without_ellipsis() {
        assert_eq!(truncate("▶ ", "hello", 20), "▶ hello");
    }

    #[test]
    fn truncate_adds_ellipsis_when_too_long() {
        let result = truncate("  ", "verylongname", 8);
        assert!(result.ends_with('…'), "expected ellipsis, got: {result}");
        assert!(result.chars().count() <= 8, "expected len ≤ 8, got: {}", result.chars().count());
    }

    #[test]
    fn truncate_exact_fit() {
        // prefix(2) + name(3) = 5 == max_w — no ellipsis needed
        assert_eq!(truncate("  ", "abc", 5), "  abc");
    }

    #[test]
    fn run_state_char_values() {
        assert_eq!(run_state_char(RunState::Running), "●");
        assert_eq!(run_state_char(RunState::Stopped), "○");
        assert_eq!(run_state_char(RunState::Failed),  "✕");
        assert_eq!(run_state_char(RunState::Missing), "·");
    }

    #[test]
    fn run_state_color_running_is_green() {
        assert_eq!(run_state_color(RunState::Running), Color::Green);
    }

    #[test]
    fn run_state_color_failed_is_red() {
        assert_eq!(run_state_color(RunState::Failed), Color::Red);
    }
}
