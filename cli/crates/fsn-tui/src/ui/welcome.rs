// Welcome screen — shown when no project exists.
//
// ┌──────────────────────────────────────────────────────────────────┐
// │  FreeSynergy.Node v0.1.0                              [DE] [q]  │
// ├──────────────────────────────────────────────────────────────────┤
// │                                                                  │
// │             Willkommen bei FreeSynergy.Node                      │
// │         Dezentrale Infrastruktur — frei und selbst betrieben     │
// │                                                                  │
// │  ┌─ System ──────────────────────────────────────────────────┐  │
// │  │  Host          : meinserver    Podman    : 5.2.1           │  │
// │  │  Benutzer      : kal           Laufzeit  : 3d 12h          │  │
// │  │  IP-Adresse    : 192.168.1.1   Architektur: x86_64         │  │
// │  │  Arbeitsspeicher: 4.2/16.0 GB  CPU-Kerne  : 8             │  │
// │  └───────────────────────────────────────────────────────────┘  │
// │                                                                  │
// │       ┌─────────────────────┐   ┌─────────────────────┐        │
// │       │   Neues Projekt     │   │ Vorhandenes Projekt  │        │
// │       └─────────────────────┘   └─────────────────────┘        │
// │                                                                  │
// ├──────────────────────────────────────────────────────────────────┤
// │  ←→=Auswahl  Enter=Bestätigen  L=Sprache  q=Beenden             │
// └──────────────────────────────────────────────────────────────────┘

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::ui::widgets;

pub fn render(f: &mut Frame, state: &AppState) {
    let area = f.area();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Min(1),     // body
            Constraint::Length(1),  // hint bar
        ])
        .split(area);

    render_header(f, state, outer[0]);
    render_body(f, state, outer[1]);
    render_hint(f, state, outer[2]);
}

// ── Header ────────────────────────────────────────────────────────────────────

fn render_header(f: &mut Frame, state: &AppState, area: Rect) {
    let title = Line::from(vec![
        Span::styled(" FreeSynergy.Node ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("v0.1.0",             Style::default().fg(Color::DarkGray)),
    ]);

    let header = Paragraph::new(title)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)))
        .alignment(Alignment::Left);
    f.render_widget(header, area);

    // Lang button — top right
    let lang_area = Rect { x: area.right().saturating_sub(6), y: area.y + 1, width: 4, height: 1 };
    f.render_widget(Paragraph::new(Line::from(widgets::lang_button(state))), lang_area);
}

// ── Body ──────────────────────────────────────────────────────────────────────

fn render_body(f: &mut Frame, state: &AppState, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(10),
            Constraint::Percentage(80),
            Constraint::Percentage(10),
        ])
        .split(area);

    let inner = cols[1];

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // spacer
            Constraint::Length(2),  // title + subtitle
            Constraint::Length(1),  // spacer
            Constraint::Length(6),  // sysinfo box
            Constraint::Length(2),  // spacer
            Constraint::Length(3),  // buttons
            Constraint::Min(1),
        ])
        .split(inner);

    render_title(f, state, rows[1]);
    render_sysinfo(f, state, rows[3]);
    render_buttons(f, state, rows[5]);
}

fn render_title(f: &mut Frame, state: &AppState, area: Rect) {
    let text = Text::from(vec![
        Line::from(Span::styled(state.t("welcome.title"),    Style::default().fg(Color::White).add_modifier(Modifier::BOLD))),
        Line::from(Span::styled(state.t("welcome.subtitle"), Style::default().fg(Color::DarkGray))),
    ]);
    f.render_widget(Paragraph::new(text).alignment(Alignment::Center), area);
}

// ── Sysinfo — aligned two-column table inside a bordered box ──────────────────

fn render_sysinfo(f: &mut Frame, state: &AppState, area: Rect) {
    let s = &state.sysinfo;

    // Fixed column widths for perfect alignment:
    //   col1_label(18) col1_value(18) | col2_label(14) col2_value
    const L: usize = 18;  // label column width (left half)
    const V: usize = 18;  // value column width (left half)
    const L2: usize = 14; // label column width (right half)

    let lbl  = Style::default().fg(Color::DarkGray);
    let val  = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
    let sep  = Style::default().fg(Color::DarkGray);

    let rows = vec![
        sysinfo_row(state.t("sys.host"),   &s.hostname,      state.t("sys.podman"), &s.podman_version, L, V, L2, lbl, val, sep),
        sysinfo_row(state.t("sys.user"),   &s.user,          state.t("sys.uptime"), &s.uptime_str,     L, V, L2, lbl, val, sep),
        sysinfo_row(state.t("sys.ip"),     &s.ip,            state.t("sys.arch"),   &s.arch,           L, V, L2, lbl, val, sep),
        sysinfo_row(state.t("sys.ram"),    &s.ram_str(),     state.t("sys.cpu"),    &s.cpu_cores.to_string(), L, V, L2, lbl, val, sep),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(" System ", Style::default().fg(Color::DarkGray)));

    let p = Paragraph::new(Text::from(rows)).block(block);
    f.render_widget(p, area);
}

#[allow(clippy::too_many_arguments)]
fn sysinfo_row(
    l1: &str, v1: &str,
    l2: &str, v2: &str,
    lw: usize, vw: usize, lw2: usize,
    lbl: Style, val: Style, sep: Style,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<lw$}", l1, lw = lw), lbl),
        Span::styled(": ", sep),
        Span::styled(format!("{:<vw$}", v1, vw = vw), val),
        Span::styled(format!("  {:<lw2$}", l2, lw2 = lw2), lbl),
        Span::styled(": ", sep),
        Span::styled(v2.to_string(), val),
    ])
}

// ── Buttons ───────────────────────────────────────────────────────────────────

fn render_buttons(f: &mut Frame, state: &AppState, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(15),
            Constraint::Percentage(30),
            Constraint::Percentage(10),
            Constraint::Percentage(30),
            Constraint::Percentage(15),
        ])
        .split(area);

    let btn1_focused = state.welcome_focus == 0;
    let btn1 = Paragraph::new(widgets::button_line(state.t("welcome.new_project"), btn1_focused, false))
        .block(Block::default().borders(Borders::ALL).border_style(
            if btn1_focused { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::DarkGray) }
        ))
        .alignment(Alignment::Center);
    f.render_widget(btn1, cols[1]);

    let btn2_focused = state.welcome_focus == 1;
    let btn2_label = format!("{} {}", state.t("welcome.open_project"), state.t("welcome.open_disabled"));
    let btn2 = Paragraph::new(widgets::button_line(&btn2_label, btn2_focused, true))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::DarkGray)))
        .alignment(Alignment::Center);
    f.render_widget(btn2, cols[3]);
}

// ── Hint bar ──────────────────────────────────────────────────────────────────

fn render_hint(f: &mut Frame, state: &AppState, area: Rect) {
    let hint = Paragraph::new(Line::from(Span::styled(
        state.t("welcome.hint"),
        Style::default().fg(Color::DarkGray),
    )))
    .alignment(Alignment::Center);
    f.render_widget(hint, area);
}
