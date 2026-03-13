// Welcome overlay — centered popup shown when no project exists.
//
// Floats over the normal layout (header/nav/footer remain visible and usable).
// The sidebar behind the popup is visible but dimmed.
//
// ┌──────────────────────────────────────────────────────────────────┐
// │  HEADER (normal)                                                 │
// ├──────────────────────────────────────────────────────────────────┤
// │  NAV-BAR (normal)                                                │
// ├──────────────────────────────────────────────────────────────────┤
// │         ╔══════════════════════════════════════════════╗         │
// │         ║  Willkommen bei FreeSynergy.Node             ║         │
// │         ║  Dezentrale Infrastruktur — frei & selbst    ║         │
// │         ║                                               ║         │
// │         ║  ┌─ System ───────────────────────────────┐  ║         │
// │         ║  │  Host: ...   Podman: ...               │  ║         │
// │         ║  └───────────────────────────────────────-─┘  ║         │
// │         ║                                               ║         │
// │         ║  [ Neues Projekt ]   [ Projekt öffnen ]      ║         │
// │         ╚══════════════════════════════════════════════╝         │
// ├──────────────────────────────────────────────────────────────────┤
// │  FOOTER (normal)                                                 │
// └──────────────────────────────────────────────────────────────────┘

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use crate::app::{AppState, OverlayLayer};
use crate::ui::render_ctx::RenderCtx;
use crate::ui::widgets;

pub fn render(f: &mut RenderCtx<'_>, state: &AppState) {
    let focus = match state.top_overlay() {
        Some(OverlayLayer::Welcome { focus }) => *focus,
        _ => return,
    };

    let (popup, rows) = popup_layout(f.area());

    // Clear background + draw border
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(block, popup);

    render_title(f, state, rows[0]);
    render_sysinfo(f, state, rows[2]);
    render_buttons(f, state, rows[4], focus);
    render_hint(f, state, rows[5]);
}

// ── Shared geometry ──────────────────────────────────────────────────────────

/// Compute popup outer rect and vertical row layout.
/// Shared between render() (for drawing) and button_rects() (for click-map).
fn popup_layout(area: Rect) -> (Rect, std::rc::Rc<[Rect]>) {
    let width  = (area.width * 3 / 4).min(76).max(50);
    // rows: border(1) + title(2) + gap(1) + sysinfo(6) + gap(1) + buttons(3) + hint(1) + border(1) = 17
    let height = 17u16.min(area.height.saturating_sub(6));
    let popup  = Rect {
        x:      area.width.saturating_sub(width) / 2,
        y:      area.height.saturating_sub(height) / 2,
        width,
        height,
    };

    let inner = Rect {
        x: popup.x + 1, y: popup.y + 1,
        width: popup.width.saturating_sub(2), height: popup.height.saturating_sub(2),
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // title + subtitle
            Constraint::Length(1), // gap
            Constraint::Length(6), // sysinfo box
            Constraint::Length(1), // gap
            Constraint::Length(3), // buttons
            Constraint::Min(1),   // gap + hint
        ])
        .split(inner);

    (popup, rows)
}

/// Compute button column layout for the welcome popup.
/// Shared between render_buttons() and button_rects().
fn btn_columns(btn_row: Rect, state: &AppState) -> std::rc::Rc<[Rect]> {
    let btn1_text = state.t("welcome.new_project");
    let btn2_text = format!("{} {}", state.t("welcome.open_project"), state.t("welcome.open_disabled"));
    let btn1_w = (btn1_text.chars().count() as u16 + 6).max(22);
    let btn2_w = (btn2_text.chars().count() as u16 + 6).max(22);
    let gap    = 4u16;
    let total  = btn1_w + btn2_w + gap;
    let side   = btn_row.width.saturating_sub(total) / 2;

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(side),
            Constraint::Length(btn1_w),
            Constraint::Length(gap),
            Constraint::Length(btn2_w),
            Constraint::Min(0),
        ])
        .split(btn_row)
}

/// Returns `(btn1_rect, btn2_rect)` for the welcome popup buttons.
/// Called from `ui/mod.rs::render()` to register in the click-map.
pub fn button_rects(area: Rect, state: &AppState) -> (Rect, Rect) {
    let (_popup, rows) = popup_layout(area);
    let cols = btn_columns(rows[4], state);
    (cols[1], cols[3])
}

// ── Sub-renderers ────────────────────────────────────────────────────────────

fn render_title(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let text = Text::from(vec![
        Line::from(Span::styled(
            state.t("welcome.title"),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            state.t("welcome.subtitle"),
            Style::default().fg(Color::DarkGray),
        )),
    ]);
    f.render_stateful_widget(
        Paragraph::new(text).alignment(Alignment::Center),
        area,
        &mut ParagraphState::new(),
    );
}

fn render_sysinfo(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let s = &state.sysinfo;

    const L: usize  = 16;
    const V: usize  = 16;
    const L2: usize = 12;

    let lbl = Style::default().fg(Color::DarkGray);
    let val = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
    let sep = Style::default().fg(Color::DarkGray);

    let rows = vec![
        sysinfo_row(state.t("sys.host"),  &s.hostname,            state.t("sys.podman"), &s.podman_version,       L, V, L2, lbl, val, sep),
        sysinfo_row(state.t("sys.user"),  &s.user,                state.t("sys.uptime"), &s.uptime_str,           L, V, L2, lbl, val, sep),
        sysinfo_row(state.t("sys.ip"),    &s.ip,                  state.t("sys.arch"),   &s.arch,                 L, V, L2, lbl, val, sep),
        sysinfo_row(state.t("sys.ram"),   &s.ram_str(),           state.t("sys.cpu"),    &s.cpu_cores.to_string(), L, V, L2, lbl, val, sep),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(" System ", Style::default().fg(Color::DarkGray)));

    f.render_stateful_widget(
        Paragraph::new(Text::from(rows)).block(block),
        area,
        &mut ParagraphState::new(),
    );
}

#[allow(clippy::too_many_arguments)]
fn sysinfo_row(
    l1: &str, v1: &str, l2: &str, v2: &str,
    lw: usize, vw: usize, lw2: usize,
    lbl: Style, val: Style, sep: Style,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<lw$}", l1), lbl),
        Span::styled(": ", sep),
        Span::styled(format!("{:<vw$}", v1), val),
        Span::styled(format!("  {:<lw2$}", l2), lbl),
        Span::styled(": ", sep),
        Span::styled(v2.to_string(), val),
    ])
}

fn render_buttons(f: &mut RenderCtx<'_>, state: &AppState, area: Rect, focus: usize) {
    let btn1_text = state.t("welcome.new_project");
    let btn2_text = format!("{} {}", state.t("welcome.open_project"), state.t("welcome.open_disabled"));

    let cols = btn_columns(area, state);

    let btn1_focused = focus == 0;
    f.render_stateful_widget(
        Paragraph::new(widgets::button_line(btn1_text, btn1_focused, false))
            .block(Block::default().borders(Borders::ALL).border_style(
                if btn1_focused { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) }
            ))
            .alignment(Alignment::Center),
        cols[1],
        &mut ParagraphState::new(),
    );

    let btn2_focused = focus == 1;
    f.render_stateful_widget(
        Paragraph::new(widgets::button_line(&btn2_text, btn2_focused, true))
            .block(Block::default().borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)))
            .alignment(Alignment::Center),
        cols[3],
        &mut ParagraphState::new(),
    );
}

fn render_hint(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    f.render_stateful_widget(
        Paragraph::new(Line::from(Span::styled(
            state.t("welcome.hint"),
            Style::default().fg(Color::DarkGray),
        )))
        .alignment(Alignment::Center),
        area,
        &mut ParagraphState::new(),
    );
}
