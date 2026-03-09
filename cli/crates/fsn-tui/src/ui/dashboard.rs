// Dashboard screen — sidebar (projects + hosts + services) + context center panel.
//
// Design Pattern: Composite — each SidebarItem renders its own sidebar line and
// center detail view. This module is the layout coordinator only; detail panel
// renderers live in ui/detail.rs and shared helpers in ui/widgets.rs.
//
// ┌──────────────────────────────────────────────────────────────────┐
// │  FSN · myproject @ example.com                          [DE]    │
// ├────────────────────┬─────────────────────────────────────────────┤
// │ PROJEKTE           │  Services                                   │
// │ ▶ myproject        │  Name      Typ    Domain    Status          │
// │   testprojekt      │▶ kanidm    iam    auth.ex   ● Aktiv        │
// │ + Neues Projekt    │  forgejo   git    git.ex    ○ Stopp        │
// │ HOSTS              │                                             │
// │   ⊡ srv1           │  (center shows details of selected item)   │
// │ + Neuer Host       │                                             │
// │ SERVICES           │                                             │
// │   ◆ kanidm         │                                             │
// │   ◆ forgejo        │                                             │
// │ + Neuer Service    │                                             │
// ├────────────────────┴─────────────────────────────────────────────┤
// │  ↑↓=Nav  n=Neu  e=Bearbeiten  x=Löschen  Tab=Detail  q=Quit     │
// └──────────────────────────────────────────────────────────────────┘

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::app::{AppState, DashFocus, Lang, SidebarItem};
use crate::ui::{detail, widgets};

pub fn render(f: &mut Frame, state: &mut AppState, area: ratatui::layout::Rect) {

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(f, state, outer[0]);
    render_body(f, state, outer[1]);
    render_hint(f, state, outer[2]);
}

// ── Header ────────────────────────────────────────────────────────────────────

fn render_header(f: &mut Frame, state: &AppState, area: Rect) {
    let (name, domain) = state.projects.get(state.selected_project)
        .map(|p| (p.name(), p.domain()))
        .unwrap_or(("FreeSynergy.Node", ""));

    let title = if domain.is_empty() {
        Line::from(vec![
            Span::styled(" FSN ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("· ", Style::default().fg(Color::DarkGray)),
            Span::styled(name.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" FSN ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("· ", Style::default().fg(Color::DarkGray)),
            Span::styled(name.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(" @ ", Style::default().fg(Color::DarkGray)),
            Span::styled(domain.to_string(), Style::default().fg(Color::DarkGray)),
        ])
    };

    let header = Paragraph::new(title)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)))
        .alignment(Alignment::Left);
    f.render_widget(header, area);

    let build_str = format!("v{} {} ({})  ", env!("CARGO_PKG_VERSION"), crate::BUILD_TIME, crate::GIT_HASH);
    let build_w   = build_str.chars().count() as u16;
    let build_x   = area.right().saturating_sub(build_w + 5);
    let build_area = Rect { x: build_x, y: area.y + 1, width: build_w, height: 1 };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(build_str, Style::default().fg(Color::DarkGray)))),
        build_area,
    );

    let lang_area = Rect { x: area.right().saturating_sub(6), y: area.y + 1, width: 4, height: 1 };
    f.render_widget(Paragraph::new(Line::from(widgets::lang_button(state))), lang_area);
}

// ── Body ──────────────────────────────────────────────────────────────────────

fn render_body(f: &mut Frame, state: &AppState, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28),
            Constraint::Min(1),
        ])
        .split(area);

    render_sidebar(f, state, cols[0]);
    render_center(f, state, cols[1]);
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

fn render_sidebar(f: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.dash_focus == DashFocus::Sidebar;

    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    f.render_widget(
        Block::default().borders(Borders::RIGHT).border_style(border_style),
        area,
    );

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    // When filter is active: reserve top row for the search input.
    let (list_area, filter_row) = if let Some(ref query) = state.sidebar_filter {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner);
        (rows[1], Some((rows[0], query.as_str())))
    } else {
        (inner, None)
    };

    // Render filter input line.
    if let Some((farea, query)) = filter_row {
        let display = format!("/{}_", query);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                display,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ))),
            farea,
        );
    }

    // Items to display: full list or filtered subset.
    let visible: Vec<(usize, &SidebarItem)> = if state.sidebar_filter.is_some() {
        state.visible_sidebar_items()
    } else {
        state.sidebar_items.iter().enumerate().collect()
    };

    if visible.is_empty() {
        let msg = if state.sidebar_filter.as_deref().is_some_and(|f| !f.is_empty()) {
            state.t("dash.filter.empty")
        } else {
            state.t("dash.no_projects")
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)))),
            list_area,
        );
        return;
    }

    let max_w = list_area.width.saturating_sub(4) as usize;

    // Each SidebarItem renders its own sidebar line — no external dispatch.
    let lines: Vec<Line> = visible.iter()
        .map(|(i, item)| item.sidebar_line(*i == state.sidebar_cursor, focused, max_w, state.lang))
        .collect();

    f.render_widget(Paragraph::new(lines), list_area);
}

// ── Center panel ──────────────────────────────────────────────────────────────

/// Dispatches to the center panel appropriate for the currently focused sidebar item.
/// Each SidebarItem knows how to render its own center view.
fn render_center(f: &mut Frame, state: &AppState, area: Rect) {
    match state.current_sidebar_item() {
        Some(item) => item.render_center(f, state, area),
        None       => render_services(f, state, area),
    }
}

// ── Services table ────────────────────────────────────────────────────────────

fn render_services(f: &mut Frame, state: &AppState, area: Rect) {
    let services_focused = state.dash_focus == DashFocus::Services;

    let block = Block::default()
        .borders(Borders::NONE)
        .title(Span::styled(
            format!(" {} ", state.t("dash.services")),
            Style::default()
                .fg(if services_focused { Color::Cyan } else { Color::White })
                .add_modifier(Modifier::BOLD),
        ));

    if state.services.is_empty() {
        let msg = Paragraph::new(Line::from(Span::styled(
            "(keine Services)",
            Style::default().fg(Color::DarkGray),
        )))
        .block(block);
        f.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec![
        Cell::from(state.t("dash.col.name"))  .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)),
        Cell::from(state.t("dash.col.type"))  .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)),
        Cell::from(state.t("dash.col.domain")).style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)),
        Cell::from(state.t("dash.col.status")).style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)),
    ])
    .height(1);

    let multi_select = !state.selected_services.is_empty();

    let rows: Vec<Row> = state.services.iter().enumerate().map(|(i, svc)| {
        let is_cursor   = i == state.selected && services_focused;
        let is_checked  = state.selected_services.contains(&i);

        let name_style = if is_cursor {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else if is_checked {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let prefix = if multi_select {
            if is_checked  { "[✓] " } else { "[ ] " }
        } else if is_cursor {
            "▶ "
        } else {
            "  "
        };

        Row::new(vec![
            Cell::from(format!("{}{}", prefix, svc.name)).style(name_style),
            Cell::from(svc.service_type.as_str()).style(Style::default().fg(Color::DarkGray)),
            Cell::from(svc.domain.as_str()).style(Style::default().fg(Color::Blue)),
            Cell::from(Line::from(widgets::status_span(svc.status, state))),
        ])
        .height(1)
    }).collect();

    let table = Table::new(rows, [
        Constraint::Length(20),
        Constraint::Length(10),
        Constraint::Min(25),
        Constraint::Length(14),
    ])
    .header(header)
    .block(block)
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    let mut table_state = TableState::default().with_selected(
        if services_focused { Some(state.selected) } else { None }
    );
    f.render_stateful_widget(table, area, &mut table_state);
}

// ── Hint bar ──────────────────────────────────────────────────────────────────

fn render_hint(f: &mut Frame, state: &AppState, area: Rect) {
    let has_confirm = state.confirm_overlay().is_some();

    // Multi-select mode hint takes priority in services focus.
    if !has_confirm && state.dash_focus == DashFocus::Services && !state.selected_services.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                state.t("dash.hint.multiselect"),
                Style::default().fg(Color::Yellow),
            ))).alignment(Alignment::Center),
            area,
        );
        return;
    }

    // Filter mode hint takes priority over confirm and normal sidebar hints.
    if !has_confirm && state.dash_focus == DashFocus::Sidebar && state.sidebar_filter.is_some() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                state.t("dash.hint.filter"),
                Style::default().fg(Color::Yellow),
            ))).alignment(Alignment::Center),
            area,
        );
        return;
    }

    let key: &'static str = if has_confirm {
        "dash.hint.confirm"
    } else {
        match state.dash_focus {
            DashFocus::Services => "dash.hint.services",
            // Each SidebarItem knows its own hint key — no external dispatch needed.
            DashFocus::Sidebar  => state.current_sidebar_item()
                .map(|item| item.hint_key())
                .unwrap_or("dash.hint"),
        }
    };

    let style = if has_confirm {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(state.t(key), style)))
            .alignment(Alignment::Center),
        area,
    );
}

// ── SidebarItem rendering — each item renders itself ─────────────────────────
//
// Design Pattern: Composite — SidebarItem is the component interface.
// Each variant implements sidebar_line() and render_center() for itself.
// The caller (render_sidebar / render_center) never needs a match block.

impl SidebarItem {
    /// Produce the sidebar row line for this item.
    ///
    /// Analogous to an element rendering its own `<li>` — the caller just
    /// collects lines; no variant-specific logic leaks into the sidebar renderer.
    pub(crate) fn sidebar_line(&self, is_cursor: bool, focused: bool, max_w: usize, lang: Lang) -> Line<'static> {
        let t = |key| crate::i18n::t(lang, key);
        match self {
            SidebarItem::Section(key) => Line::from(Span::styled(
                t(key),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED),
            )),

            SidebarItem::Project { name, .. } => {
                let (prefix, style) = if is_cursor {
                    ("▶ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                } else {
                    ("  ", Style::default().fg(Color::White))
                };
                Line::from(Span::styled(widgets::truncate(prefix, name, max_w), style))
            }

            SidebarItem::Host { name, .. } => {
                let (prefix, style) = if is_cursor {
                    ("  ▶ ", Style::default().fg(Color::Cyan))
                } else {
                    ("  ⊡ ", Style::default().fg(Color::DarkGray))
                };
                Line::from(Span::styled(widgets::truncate(prefix, name, max_w), style))
            }

            SidebarItem::Service { name, status, .. } => {
                // widgets::run_state_char/color — single source of truth.
                let status_char  = widgets::run_state_char(*status);
                let status_color = widgets::run_state_color(*status);
                let (prefix, name_style) = if is_cursor {
                    ("  ▶ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                } else {
                    ("  ◆ ", Style::default().fg(Color::White))
                };
                let text = widgets::truncate(prefix, name, max_w.saturating_sub(2));
                Line::from(vec![
                    Span::styled(text, name_style),
                    Span::styled(format!(" {}", status_char), Style::default().fg(status_color)),
                ])
            }

            SidebarItem::Action { label_key, .. } => {
                let style = if is_cursor {
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else if focused {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                Line::from(Span::styled(t(label_key), style))
            }
        }
    }

    /// Render the center detail panel appropriate for this item's type.
    ///
    /// Delegates to ui/detail.rs renderers — the caller only knows
    /// "show the center panel for the selected item".
    pub(crate) fn render_center(&self, f: &mut Frame, state: &AppState, area: Rect) {
        match self {
            SidebarItem::Project { slug, .. } => detail::render_project_detail(f, state, area, slug),
            SidebarItem::Host    { slug, .. } => detail::render_host_detail(f, state, area, slug),
            SidebarItem::Service { name, .. } => detail::render_service_detail(f, state, area, name),
            // Action, Section → show service table
            _                                 => render_services(f, state, area),
        }
    }
}
