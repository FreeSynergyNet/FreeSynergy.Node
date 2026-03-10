// Dashboard screen — modern 3-zone layout.
//
// Design Pattern: Composite — each SidebarItem renders its own sidebar line and
// center detail view. This module is the layout coordinator only.
//
// ┌──────────────────────────────────────────────────────────────────────┐
// │ [BigText FSN]  FreeSynergy.Node                         v0.1  [DE]  │ ← Header (5 rows)
// │ [          ]  Modular Deployment System  —  by KalEl               │
// │ [          ]  myproject @ example.com                               │
// │ [          ]  ──────────────────────────────────────────────────── │
// │  [Projekte]│[Hosts]│[Services]│[Store]│[⚙ Einstellungen]           │ ← Tab bar
// ├──────────────┬───────────────────────────────────────────────────── ┤
// │              │ ╭──────────╮╭──────────╮╭──────────╮╭──────────╮   │ ← Stats cards (3 rows)
// │  Sidebar     │ │  RAM     ││  System  ││ Running  ││  Alerts  │   │
// │  1/3         │ ╰──────────╯╰──────────╯╰──────────╯╰──────────╯   │
// │              │ Services / Detail / Env vars                         │ ← Content
// ├──────────────┴──────────────────────────────────────────────────────┤
// │  MIT © FreeSynergy.Net             ↑↓=Nav  F1=Hilfe  q=Ende        │ ← Footer (1 row)
// └──────────────────────────────────────────────────────────────────────┘
//
// F1 help panel slides in from the right within the body only.
// Header and footer remain unaffected.

use tui_big_text::{BigText, PixelSize};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::ui::render_ctx::RenderCtx;

use fsn_core::health::HealthLevel;
use crate::app::{AppState, DashFocus, Lang, RunState, SidebarAction, SidebarItem};
use crate::ui::{detail, help_sidebar, widgets};

// ── Navigation tab labels ─────────────────────────────────────────────────────

const TAB_KEYS: &[&str] = &[
    "dash.tab.projects",
    "dash.tab.hosts",
    "dash.tab.services",
    "dash.tab.store",
    "dash.tab.settings",
];

// ── Main render ───────────────────────────────────────────────────────────────

pub fn render(f: &mut RenderCtx<'_>, state: &mut AppState, area: Rect) {
    let zones = Layout::vertical([
        Constraint::Length(5), // header: 4 logo rows + 1 tab bar
        Constraint::Min(1),    // body
        Constraint::Length(1), // footer
    ])
    .split(area);

    render_header(f, state, zones[0]);
    render_body(f, state, zones[1]);
    render_footer(f, state, zones[2]);
}

// ── Header ────────────────────────────────────────────────────────────────────

fn render_header(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let rows = Layout::vertical([
        Constraint::Length(4), // logo row (BigText Quadrant = 4 rows)
        Constraint::Length(1), // tab bar
    ])
    .split(area);

    render_logo_row(f, state, rows[0]);
    render_tab_bar(f, state, rows[1]);
}

fn render_logo_row(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    // Left: BigText "FSN" | Right: title, subtitle, project info, separator
    let cols = Layout::horizontal([
        Constraint::Length(18), // "FSN" in Quadrant: 3 chars × 4 cols + padding
        Constraint::Min(1),
    ])
    .split(area);

    // ── BigText logo ─────────────────────────────────────────────────────────
    let big = BigText::builder()
        .pixel_size(PixelSize::Quadrant)
        .style(Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .lines(vec![Line::from("FSN")])
        .build();
    f.render_widget(big, cols[0]);

    // ── Right info column (4 rows) ────────────────────────────────────────────
    let info_rows = Layout::vertical([
        Constraint::Length(1), // title + lang
        Constraint::Length(1), // subtitle + version
        Constraint::Length(1), // project/domain
        Constraint::Length(1), // separator
    ])
    .split(cols[1]);

    // Row 0: "FreeSynergy.Node" (left) + [DE] button (right)
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("FreeSynergy", Style::new().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(".Node", Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ])),
        info_rows[0],
    );
    let lang_area = Rect {
        x: cols[1].right().saturating_sub(5),
        y: info_rows[0].y,
        width: 5,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(Line::from(widgets::lang_button(state))),
        lang_area,
    );

    // Row 1: subtitle (left) + version (right)
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Modular Deployment System  —  by KalEl",
            Style::new().fg(Color::DarkGray),
        ))),
        info_rows[1],
    );
    let ver_str = format!("v{}  ", env!("CARGO_PKG_VERSION"));
    let ver_w = ver_str.chars().count() as u16;
    let ver_area = Rect {
        x: cols[1].right().saturating_sub(ver_w),
        y: info_rows[1].y,
        width: ver_w,
        height: 1,
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(ver_str, Style::new().fg(Color::DarkGray)))),
        ver_area,
    );

    // Row 2: project name @ domain
    let domain_text = state
        .projects
        .get(state.selected_project)
        .map(|p| format!("{}  @  {}", p.name(), p.domain()))
        .unwrap_or_else(|| state.t("dash.no_project_selected").to_string());
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(domain_text, Style::new().fg(Color::DarkGray)))),
        info_rows[2],
    );

    // Row 3: separator line
    f.render_widget(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::new().fg(Color::DarkGray)),
        info_rows[3],
    );
}

fn render_tab_bar(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let active = active_tab_index(state);
    let mut spans: Vec<Span> = vec![Span::raw(" ")];

    for (i, &key) in TAB_KEYS.iter().enumerate() {
        let label = state.t(key);
        if i == active {
            spans.push(Span::styled(
                format!(" {} ", label),
                Style::new()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {} ", label),
                Style::new().fg(Color::DarkGray),
            ));
        }
        if i < TAB_KEYS.len() - 1 {
            spans.push(Span::styled(" │ ", Style::new().fg(Color::DarkGray)));
        }
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn active_tab_index(state: &AppState) -> usize {
    match state.current_sidebar_item() {
        Some(SidebarItem::Project { .. }) => 0,
        Some(SidebarItem::Host { .. }) => 1,
        Some(SidebarItem::Service { .. }) => 2,
        Some(SidebarItem::Action { kind, .. }) => match kind {
            SidebarAction::NewProject => 0,
            SidebarAction::NewHost => 1,
            SidebarAction::NewService => 2,
        },
        _ => 0,
    }
}

// ── Body ──────────────────────────────────────────────────────────────────────

fn render_body(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    // F1 help panel slides in from the right — header and footer stay untouched.
    let (main_area, help_opt) =
        if state.help_visible && area.width > help_sidebar::SIDEBAR_WIDTH + 40 {
            let cols = Layout::horizontal([
                Constraint::Min(40),
                Constraint::Length(help_sidebar::SIDEBAR_WIDTH),
            ])
            .split(area);
            (cols[0], Some(cols[1]))
        } else {
            (area, None)
        };

    // Sidebar (1/3 fixed) + detail panel (2/3+)
    let cols = Layout::horizontal([
        Constraint::Length(28),
        Constraint::Min(1),
    ])
    .split(main_area);

    render_sidebar(f, state, cols[0]);
    render_detail_panel(f, state, cols[1]);

    // F1 help panel (right side, body only)
    if let Some(help_area) = help_opt {
        let kind = state.current_form.as_ref().map(|f| f.kind);
        let foc_key = state
            .current_form
            .as_ref()
            .and_then(|f| f.focused_node())
            .map(|n| n.key());
        let sections = help_sidebar::build_help(state.screen, kind, foc_key, state.lang);
        help_sidebar::render_help_sidebar(f, help_area, &sections, state.lang);
    }
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

fn render_sidebar(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let focused = state.dash_focus == DashFocus::Sidebar;

    let border_style = if focused {
        Style::new().fg(Color::Cyan)
    } else {
        Style::new().fg(Color::DarkGray)
    };

    f.render_widget(
        Block::default()
            .borders(Borders::RIGHT)
            .border_style(border_style),
        area,
    );

    let inner = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };

    // Filter input row at top when active
    let (list_area, filter_row) = if let Some(ref query) = state.sidebar_filter {
        let rows = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);
        (rows[1], Some((rows[0], query.as_str())))
    } else {
        (inner, None)
    };

    if let Some((farea, query)) = filter_row {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("/{}_", query),
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ))),
            farea,
        );
    }

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
            Paragraph::new(Line::from(Span::styled(msg, Style::new().fg(Color::DarkGray)))),
            list_area,
        );
        return;
    }

    let max_w = list_area.width.saturating_sub(2) as usize;
    let lines: Vec<Line> = visible
        .iter()
        .map(|(i, item)| item.sidebar_line(*i == state.sidebar_cursor, focused, max_w, state.lang))
        .collect();
    f.render_widget(Paragraph::new(lines), list_area);
}

// ── Detail panel ──────────────────────────────────────────────────────────────

fn render_detail_panel(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    // Stats cards (top, fixed 3 rows) + content (flexible)
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .split(area);

    render_stats_cards(f, state, rows[0]);
    render_center(f, state, rows[1]);
}

// ── Stats cards ───────────────────────────────────────────────────────────────

fn render_stats_cards(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let cards = Layout::horizontal([
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ])
    .split(area);

    // Card 1: RAM usage
    render_stat_card(f, cards[0], "RAM", &state.sysinfo.ram_str(), Color::Cyan);

    // Card 2: Hostname / User
    let sys_label = format!("{}@{}", state.sysinfo.user, state.sysinfo.hostname);
    render_stat_card(f, cards[1], "System", &sys_label, Color::White);

    // Card 3: Running services
    let total = state.services.len();
    let ok = state.services.iter().filter(|s| s.status == RunState::Running).count();
    let running_color = if total == 0 {
        Color::DarkGray
    } else if ok == total {
        Color::Green
    } else {
        Color::Yellow
    };
    render_stat_card(f, cards[2], "Running", &format!("{} / {}", ok, total), running_color);

    // Card 4: Alerts (stopped + failed)
    let failed  = state.services.iter().filter(|s| s.status == RunState::Failed).count();
    let stopped = state.services.iter().filter(|s| s.status == RunState::Stopped).count();
    let alert_color = if failed > 0 { Color::Red } else if stopped > 0 { Color::Yellow } else { Color::Green };
    render_stat_card(f, cards[3], "Alerts", &format!("⚠ {}  ✗ {}", stopped, failed), alert_color);
}

fn render_stat_card(f: &mut RenderCtx<'_>, area: Rect, label: &str, value: &str, color: Color) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(Color::DarkGray))
        .title(Span::styled(
            format!(" {} ", label),
            Style::new().fg(Color::DarkGray),
        ));

    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            value.to_string(),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center),
        inner,
    );
}

// ── Center panel ──────────────────────────────────────────────────────────────

fn render_center(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    match state.current_sidebar_item() {
        Some(item) => item.render_center(f, state, area),
        None       => render_services(f, state, area),
    }
}

// ── Services table ────────────────────────────────────────────────────────────

fn render_services(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let services_focused = state.dash_focus == DashFocus::Services;

    let block = Block::default()
        .borders(Borders::NONE)
        .title(Span::styled(
            format!(" {} ", state.t("dash.services")),
            Style::new()
                .fg(if services_focused { Color::Cyan } else { Color::White })
                .add_modifier(Modifier::BOLD),
        ));

    if state.services.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                state.t("dash.no_services"),
                Style::new().fg(Color::DarkGray),
            )))
            .block(block),
            area,
        );
        return;
    }

    let header = Row::new(vec![
        Cell::from(state.t("dash.col.name"))
            .style(Style::new().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)),
        Cell::from(state.t("dash.col.type"))
            .style(Style::new().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)),
        Cell::from(state.t("dash.col.domain"))
            .style(Style::new().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)),
        Cell::from(state.t("dash.col.status"))
            .style(Style::new().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED)),
    ])
    .height(1);

    let multi_select = !state.selected_services.is_empty();

    let rows: Vec<Row> = state
        .services
        .iter()
        .enumerate()
        .map(|(i, svc)| {
            let is_cursor  = i == state.selected && services_focused;
            let is_checked = state.selected_services.contains(&i);

            let name_style = if is_cursor {
                Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if is_checked {
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(Color::White)
            };

            let prefix = if multi_select {
                if is_checked { "[✓] " } else { "[ ] " }
            } else if is_cursor {
                "▶ "
            } else {
                "  "
            };

            Row::new(vec![
                Cell::from(format!("{}{}", prefix, svc.name)).style(name_style),
                Cell::from(svc.service_type.as_str()).style(Style::new().fg(Color::DarkGray)),
                Cell::from(svc.domain.as_str()).style(Style::new().fg(Color::Blue)),
                Cell::from(Line::from(widgets::status_span(svc.status, state))),
            ])
            .height(1)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(20),
            Constraint::Length(10),
            Constraint::Min(25),
            Constraint::Length(14),
        ],
    )
    .header(header)
    .block(block)
    .row_highlight_style(Style::new().bg(Color::DarkGray));

    let mut table_state = TableState::default()
        .with_selected(if services_focused { Some(state.selected) } else { None });
    f.render_stateful_widget(table, area, &mut table_state);
}

// ── Footer ────────────────────────────────────────────────────────────────────

fn render_footer(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    // Left: MIT license
    let mit = "  MIT © FreeSynergy.Net";
    let mit_w = mit.chars().count() as u16;
    let mit_area = Rect {
        x: area.x,
        y: area.y,
        width: mit_w.min(area.width / 2),
        height: 1,
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(mit, Style::new().fg(Color::DarkGray)))),
        mit_area,
    );

    // Right: context-sensitive hints
    let has_confirm = state.confirm_overlay().is_some();

    let hint_text: String = if has_confirm {
        state.t("dash.hint.confirm").to_string()
    } else if state.dash_focus == DashFocus::Services && !state.selected_services.is_empty() {
        state.t("dash.hint.multiselect").to_string()
    } else if state.dash_focus == DashFocus::Sidebar && state.sidebar_filter.is_some() {
        state.t("dash.hint.filter").to_string()
    } else {
        let key = match state.dash_focus {
            DashFocus::Services => "dash.hint.services",
            DashFocus::Sidebar  => state
                .current_sidebar_item()
                .map(|i| i.hint_key())
                .unwrap_or("dash.hint"),
        };
        format!("{}  {}  {}  ", state.t(key), state.t("dash.hint.f1"), state.t("dash.hint.quit"))
    };

    let hint_style = if has_confirm {
        Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::DarkGray)
    };

    let hints_w = hint_text.chars().count() as u16;
    if hints_w < area.width {
        let hints_area = Rect {
            x: area.right().saturating_sub(hints_w),
            y: area.y,
            width: hints_w,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(hint_text, hint_style))),
            hints_area,
        );
    }
}

// ── SidebarItem rendering — each item renders itself ─────────────────────────
//
// Design Pattern: Composite — SidebarItem is the component interface.
// Each variant implements sidebar_line() and render_center() for itself.

impl SidebarItem {
    /// Produce the sidebar row line for this item.
    pub(crate) fn sidebar_line(
        &self,
        is_cursor: bool,
        focused: bool,
        max_w: usize,
        lang: Lang,
    ) -> Line<'static> {
        let t = |key| crate::i18n::t(lang, key);
        match self {
            SidebarItem::Section(key) => Line::from(Span::styled(
                t(key),
                Style::new().fg(Color::DarkGray).add_modifier(Modifier::UNDERLINED),
            )),

            SidebarItem::Project { name, health, .. } => {
                let (prefix, name_style) = if is_cursor {
                    ("▶ ", Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                } else {
                    ("  ", Style::new().fg(Color::White))
                };
                let indicator_style = health_color(*health);
                let text = widgets::truncate(prefix, name, max_w.saturating_sub(2));
                Line::from(vec![
                    Span::styled(text, name_style),
                    Span::styled(format!(" {}", health.indicator()), indicator_style),
                ])
            }

            SidebarItem::Host { name, health, .. } => {
                let (prefix, name_style) = if is_cursor {
                    ("  ▶ ", Style::new().fg(Color::Cyan))
                } else {
                    ("  ⊡ ", Style::new().fg(Color::DarkGray))
                };
                let indicator_style = health_color(*health);
                let text = widgets::truncate(prefix, name, max_w.saturating_sub(2));
                Line::from(vec![
                    Span::styled(text, name_style),
                    Span::styled(format!(" {}", health.indicator()), indicator_style),
                ])
            }

            SidebarItem::Service { name, status, .. } => {
                let status_char  = widgets::run_state_char(*status);
                let status_color = widgets::run_state_color(*status);
                let (prefix, name_style) = if is_cursor {
                    ("  ▶ ", Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                } else {
                    ("  ◆ ", Style::new().fg(Color::White))
                };
                let text = widgets::truncate(prefix, name, max_w.saturating_sub(2));
                Line::from(vec![
                    Span::styled(text, name_style),
                    Span::styled(format!(" {}", status_char), Style::new().fg(status_color)),
                ])
            }

            SidebarItem::Action { label_key, .. } => {
                let style = if is_cursor {
                    Style::new().fg(Color::Green).add_modifier(Modifier::BOLD)
                } else if focused {
                    Style::new().fg(Color::Green)
                } else {
                    Style::new().fg(Color::DarkGray)
                };
                Line::from(Span::styled(t(label_key), style))
            }
        }
    }

    /// Render the center detail panel for this item's type.
    pub(crate) fn render_center(&self, f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
        match self {
            SidebarItem::Project { slug, .. } => detail::render_project_detail(f, state, area, slug),
            SidebarItem::Host    { slug, .. } => detail::render_host_detail(f, state, area, slug),
            SidebarItem::Service { name, .. } => detail::render_service_detail(f, state, area, name),
            _                                 => render_services(f, state, area),
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn health_color(level: HealthLevel) -> Style {
    match level {
        HealthLevel::Ok      => Style::new().fg(Color::Green),
        HealthLevel::Warning => Style::new().fg(Color::Yellow),
        HealthLevel::Error   => Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
    }
}
