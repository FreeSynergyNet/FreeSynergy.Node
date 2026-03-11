// Settings screen — tabbed preferences panel.
//
// Pattern: Composite — each tab is a self-contained render function.
// Adding a new settings section = add SettingsTab variant + match arm here.
//
// Current tabs:
//   Stores    — module store management (add/remove/enable URLs)
//   Languages — i18n language management (view/activate/remove)
//
// Mouse registration (ClickMap):
//   Each list row registers a ClickTarget::SettingsCursor / LangCursor during render.
//   mouse.rs dispatches left-click and double-click to the correct handler.
//
// Layout:
//   ┌─────────────────────────────────────────────────────────────┐
//   │  ⚙ Settings                                                 │
//   ├── [Stores] [Languages] ─────────────────────────────────────┤
//   │  (tab content)                                              │
//   ├─────────────────────────────────────────────────────────────┤
//   │  (hint bar)                                                 │
//   └─────────────────────────────────────────────────────────────┘

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use crate::app::{AppState, SettingsTab};
use crate::click_map::{ClickMap, ClickTarget};
use crate::i18n::{TRANSLATION_API_VERSION, t};
use crate::ui::render_ctx::RenderCtx;

pub fn render(f: &mut RenderCtx<'_>, state: &mut AppState, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", state.t("settings.title")),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split: tab bar | content | hint
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab bar
            Constraint::Min(3),    // content
            Constraint::Length(1), // hint
        ])
        .split(inner);

    render_tab_bar(f, state, chunks[0]);

    // Take ClickMap — disjoint field borrow from the rest of state.
    let mut cmap = std::mem::take(&mut state.click_map);
    match state.settings_tab {
        SettingsTab::Stores    => render_stores(f, state, chunks[1], &mut cmap),
        SettingsTab::Languages => render_languages(f, state, chunks[1], &mut cmap),
    }
    state.click_map = cmap;

    render_hint(f, state, chunks[2]);
}

// ── Tab bar ───────────────────────────────────────────────────────────────────

fn render_tab_bar(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let lang = state.lang;
    let tabs: &[SettingsTab] = &[SettingsTab::Stores, SettingsTab::Languages];

    let spans: Vec<Span> = tabs.iter().flat_map(|&tab| {
        let label = t(lang, tab.label_key());
        let is_active = tab == state.settings_tab;
        let style = if is_active {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        [Span::styled(format!(" {label} "), style), Span::raw(" ")]
    }).collect();

    f.render_stateful_widget(
        Paragraph::new(Line::from(spans)),
        area,
        &mut ParagraphState::new(),
    );
}

// ── Stores tab ────────────────────────────────────────────────────────────────
//
// Each store entry is rendered individually so we can register an exact Rect
// in the ClickMap for mouse hit-testing.

fn render_stores(f: &mut RenderCtx<'_>, state: &AppState, area: Rect, cmap: &mut ClickMap) {
    let stores = &state.settings.stores;

    if stores.is_empty() {
        f.render_stateful_widget(
            Paragraph::new(Line::from(Span::styled(
                state.t("settings.empty"),
                Style::default().fg(Color::DarkGray),
            ))),
            area,
            &mut ParagraphState::new(),
        );
        return;
    }

    let mut y = area.y;
    for (i, store) in stores.iter().enumerate() {
        if y >= area.bottom() { break; }

        // Height: name+status (1) + URL (1) + optional path (1) + optional git (1) + blank (1)
        let detail_lines = store.local_path.is_some() as u16 + store.git_url.is_some() as u16;
        let item_h = (2 + detail_lines + 1).min(area.bottom().saturating_sub(y));
        let item_rect = Rect { x: area.x, y, width: area.width, height: item_h };

        cmap.push(item_rect, ClickTarget::SettingsCursor { idx: i });

        let is_sel     = i == state.settings_cursor;
        let status_key = if store.enabled { "settings.store.enabled" } else { "settings.store.disabled" };
        let status     = state.t(status_key);
        let status_col = if store.enabled { Color::Green } else { Color::DarkGray };
        let marker     = if is_sel { "▶ " } else { "  " };
        let name_style = if is_sel {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let mut lines: Vec<Line<'_>> = vec![
            Line::from(vec![
                Span::raw(marker),
                Span::styled(store.name.as_str(), name_style),
                Span::raw("  "),
                Span::styled(status, Style::default().fg(status_col)),
            ]),
            Line::from(vec![
                Span::raw("    "),
                Span::styled("URL:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(store.url.as_str(), Style::default().fg(Color::DarkGray)),
            ]),
        ];
        if let Some(ref lp) = store.local_path {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
                Span::styled(lp.as_str(), Style::default().fg(Color::Yellow)),
            ]));
        }
        if let Some(ref gu) = store.git_url {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled("Git:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(gu.as_str(), Style::default().fg(Color::DarkGray)),
            ]));
        }
        lines.push(Line::from(""));

        f.render_stateful_widget(Paragraph::new(lines), item_rect, &mut ParagraphState::new());
        y += item_h;
    }
}

// ── Languages tab ─────────────────────────────────────────────────────────────
//
// Each language row is exactly 1 line — cursor idx maps directly to y-offset.

fn render_languages(f: &mut RenderCtx<'_>, state: &AppState, area: Rect, cmap: &mut ClickMap) {
    let lang      = state.lang;
    let installed = state.available_langs.len();
    let mut lines: Vec<Line<'static>> = Vec::new();
    // Track the rendered line count so ClickMap rects align with Paragraph lines.
    let mut line_y = area.y;

    // ── English — built-in, always first (cursor index 0) ─────────────────
    {
        let is_active = matches!(state.lang, crate::app::Lang::En);
        let is_sel    = state.lang_cursor == 0;
        if line_y < area.bottom() {
            cmap.push(Rect { x: area.x, y: line_y, width: area.width, height: 1 },
                ClickTarget::LangCursor { idx: 0 });
        }
        push_lang_row(&mut lines, "EN", "English", is_active, is_sel,
            t(lang, "settings.lang.builtin").to_string(), Color::DarkGray, lang);
        line_y += 1;
    }

    // ── Installed languages ───────────────────────────────────────────────────
    if state.available_langs.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(t(lang, "settings.lang.none"), Style::default().fg(Color::DarkGray)),
        ]));
        line_y += 2;
    } else {
        for (i, dl) in state.available_langs.iter().enumerate() {
            let lang_cursor = i + 1;
            let is_active   = matches!(state.lang, crate::app::Lang::Dynamic(d) if d.code == dl.code);
            let is_sel      = state.lang_cursor == lang_cursor;

            if line_y < area.bottom() {
                cmap.push(Rect { x: area.x, y: line_y, width: area.width, height: 1 },
                    ClickTarget::LangCursor { idx: lang_cursor });
            }
            line_y += 1;

            let (api_label, api_color) = if dl.api_version == TRANSLATION_API_VERSION {
                (t(lang, "settings.lang.api_ok"), Color::Green)
            } else {
                (t(lang, "settings.lang.api_warn"), Color::Yellow)
            };
            let info = format!("{}%  {}", dl.completeness, api_label);
            push_lang_row(&mut lines, dl.code_upper, dl.name, is_active, is_sel,
                info, api_color, lang);
        }
    }

    // ── Available for download (store languages not yet installed) ────────────
    let downloadable: Vec<(&str, &str)> = crate::KNOWN_STORE_LANGS.iter()
        .filter(|(code, _)| !state.available_langs.iter().any(|d| d.code == *code))
        .map(|&(c, n)| (c, n))
        .collect();

    if !downloadable.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("Available in Store", Style::default().fg(Color::DarkGray)
                .add_modifier(Modifier::UNDERLINED)),
        ]));
        line_y += 2; // blank + section header (not clickable)

        for (i, (code, name)) in downloadable.iter().enumerate() {
            let dl_cursor = 1 + installed + i;
            let is_sel    = state.lang_cursor == dl_cursor;

            if line_y < area.bottom() {
                cmap.push(Rect { x: area.x, y: line_y, width: area.width, height: 1 },
                    ClickTarget::LangCursor { idx: dl_cursor });
            }
            line_y += 1;

            let marker     = if is_sel { "▶ " } else { "  " };
            let name_style = if is_sel {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let code_upper = code.to_uppercase();
            lines.push(Line::from(vec![
                Span::raw(marker),
                Span::styled(format!("[{code_upper}] "), Style::default().fg(Color::DarkGray)),
                Span::styled(name.to_string(),           name_style),
                Span::raw("  "),
                Span::styled("↓ Enter/F to download",    Style::default().fg(Color::Yellow)),
            ]));
        }
    }

    let _ = line_y; // suppress unused warning
    f.render_stateful_widget(Paragraph::new(lines), area, &mut ParagraphState::new());
}

fn push_lang_row(
    lines:     &mut Vec<Line<'static>>,
    code:      &'static str,
    name:      &'static str,
    is_active: bool,
    is_sel:    bool,
    info:      String,
    info_col:  Color,
    lang:      crate::app::Lang,
) {
    let marker     = if is_sel { "▶ " } else { "  " };
    let status_key = if is_active { "settings.lang.active" } else { "settings.lang.inactive" };
    let status     = t(lang, status_key);
    let status_col = if is_active { Color::Green } else { Color::DarkGray };
    let name_style = if is_sel {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    lines.push(Line::from(vec![
        Span::raw(marker),
        Span::styled(format!("[{code}] "), Style::default().fg(Color::Yellow)),
        Span::styled(name, name_style),
        Span::raw("  "),
        Span::styled(status, Style::default().fg(status_col)),
        Span::raw("  "),
        Span::styled(info, Style::default().fg(info_col)),
    ]));
}

// ── Hint bar ─────────────────────────────────────────────────────────────────

fn render_hint(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let key = match state.settings_tab {
        SettingsTab::Stores    => "settings.stores.hint",
        SettingsTab::Languages => "settings.lang.hint",
    };
    f.render_stateful_widget(
        Paragraph::new(Line::from(Span::styled(
            state.t(key),
            Style::default().fg(Color::DarkGray),
        ))),
        area,
        &mut ParagraphState::new(),
    );
}
