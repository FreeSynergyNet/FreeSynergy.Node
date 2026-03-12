// Settings screen — sidebar-first layout consistent with the Dashboard.
//
// Design Pattern: Composite — left sidebar (section navigation) + right content panel.
// Each SettingsSection has its own content render function.
//
// Layout (reuses AppLayout + HeaderBar + FooterBar):
//   ┌────────────────────────────────────────────────────────────┐
//   │  HeaderBar (5 rows)                                        │
//   ├──────────────┬─────────────────────────────────────────────┤
//   │  Sidebar     │  Content panel                              │
//   │  ▶ Stores    │  (section-specific items)                   │
//   │    Languages │                                             │
//   │    General   │                                             │
//   │    About     │  hint bar at content bottom                 │
//   ├──────────────┴─────────────────────────────────────────────┤
//   │  FooterBar (1 row)                                         │
//   └────────────────────────────────────────────────────────────┘
//
// Mouse registration (ClickMap):
//   SettingsSidebar { idx } — each sidebar section row
//   SettingsCursor  { idx } — each Stores content row
//   LangCursor      { idx } — each Languages content row
//
// Adding a new section:
//   1. Add variant to SettingsSection in app/screen.rs.
//   2. Add label_key() arm in SettingsSection::label_key().
//   3. Add render_* function below.
//   4. Add match arm in render_content() and render_hint().
//   5. Add keyboard handler in events.rs::handle_settings_content().

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use crate::app::{AppState, SettingsFocus, SettingsSection, StoreSettingsFocus};
use crate::click_map::{ClickMap, ClickTarget};
use crate::i18n::{TRANSLATION_API_VERSION, t};
use crate::ui::components::{Component, FooterBar, HeaderBar};
use crate::ui::layout::{AppLayout, LayoutConfig};
use crate::ui::render_ctx::RenderCtx;

/// Width of the settings sidebar column in characters.
const SIDEBAR_WIDTH: u16 = 22;

pub fn render(f: &mut RenderCtx<'_>, state: &mut AppState, area: Rect) {
    // Clear last frame's ClickMap — HeaderBar + content will rebuild it.
    state.click_map.clear();

    let layout = AppLayout::compute(area, &LayoutConfig {
        topbar_height: 5,
        left_width:    Some(SIDEBAR_WIDTH),
        ..LayoutConfig::default()
    });

    HeaderBar.render(f, layout.topbar, state);
    FooterBar.render(f, layout.footer_primary, state);

    let sidebar_area = layout.body.left.unwrap_or(layout.body.main);
    let content_area = layout.body.main;

    // Take ClickMap — avoids simultaneous borrow of state + state.click_map.
    let mut cmap = std::mem::take(&mut state.click_map);
    render_sidebar(f, state, sidebar_area, &mut cmap);
    render_content(f, state, content_area, &mut cmap);
    state.click_map = cmap;
}

// ── Settings sidebar ──────────────────────────────────────────────────────────

fn render_sidebar(f: &mut RenderCtx<'_>, state: &AppState, area: Rect, cmap: &mut ClickMap) {
    let focused = state.settings_focus == SettingsFocus::Sidebar;
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
        x:      area.x + 1,
        y:      area.y,
        width:  area.width.saturating_sub(2),
        height: area.height,
    };

    let lang  = state.lang;
    let lines: Vec<Line> = SettingsSection::ALL.iter().enumerate().map(|(i, &sec)| {
        let is_cursor = i == state.settings_sidebar_cursor;
        let is_active = sec == state.settings_section;

        // Register click target for this row.
        let row_y = inner.y + i as u16;
        if row_y < area.bottom() {
            cmap.push(
                Rect { x: inner.x, y: row_y, width: inner.width, height: 1 },
                ClickTarget::SettingsSidebar { idx: i },
            );
        }

        let marker = if is_cursor && focused { "▶ " } else { "  " };
        let label  = t(lang, sec.label_key());
        let style  = if is_active && focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else if is_cursor && focused {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        Line::from(vec![
            Span::raw(marker),
            Span::styled(label.to_string(), style),
        ])
    }).collect();

    f.render_stateful_widget(Paragraph::new(lines), inner, &mut ParagraphState::new());
}

// ── Content dispatcher ────────────────────────────────────────────────────────

fn render_content(f: &mut RenderCtx<'_>, state: &AppState, area: Rect, cmap: &mut ClickMap) {
    // Left border separates content from sidebar.
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split content into: main area + 1-row hint.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    match state.settings_section {
        SettingsSection::General   => render_general(f, state, chunks[0]),
        SettingsSection::Store     => render_store(f, state, chunks[0], cmap),
        SettingsSection::Languages => render_languages(f, state, chunks[0], cmap),
        SettingsSection::About     => render_about(f, state, chunks[0]),
    }

    render_hint(f, state, chunks[1]);
}

// ── Store section (Settings → Store) ─────────────────────────────────────────
//
// Layout:
//   ┌─ Repositories ──────────────────────────────────┐
//   │ ✓ FSN Official    https://raw.github.com/..     │
//   │   My Custom       https://example.com/..        │
//   ├─────────────────────────────────────────────────┤
//   │ Modules                                         │
//   │ [x] proxy/zentinel  ✓                           │
//   │ [ ] iam/kanidm                                  │
//   └─────────────────────────────────────────────────┘
//   Footer hint: Ctrl+A: Apply pending changes

fn render_store(f: &mut RenderCtx<'_>, state: &AppState, area: Rect, cmap: &mut ClickMap) {
    // Split vertically: repos top half, modules bottom half.
    let repos_height = (state.settings.stores.len() as u16 + 2).min(area.height / 2);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(repos_height),
            Constraint::Min(1),
        ])
        .split(area);

    render_store_repos(f, state, chunks[0], cmap);
    render_store_modules(f, state, chunks[1], cmap);
}

fn render_store_repos(f: &mut RenderCtx<'_>, state: &AppState, area: Rect, cmap: &mut ClickMap) {
    let stores  = &state.settings.stores;
    let focused = state.settings_focus == SettingsFocus::Content
        && state.settings_section == SettingsSection::Store
        && state.settings_store_focus == StoreSettingsFocus::Repos;

    // Header row
    let header_line = Line::from(vec![
        Span::styled(
            state.t("settings.store.tab.repos"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  [+] Add", Style::default().fg(Color::DarkGray)),
    ]);

    let mut lines: Vec<Line<'_>> = vec![header_line];

    if stores.is_empty() {
        lines.push(Line::from(Span::styled(
            state.t("settings.empty"),
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, store) in stores.iter().enumerate() {
            let is_sel     = focused && i == state.settings_cursor;
            let enabled    = store.enabled;
            let status_sym = if enabled { "✓" } else { " " };
            let status_col = if enabled { Color::Green } else { Color::DarkGray };
            let marker     = if is_sel { "▶ " } else { "  " };
            let name_style = if is_sel {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let url_short: String = store.url.chars().take(50).collect();

            let row_y = area.y + i as u16 + 1; // +1 for header
            if row_y < area.bottom() {
                cmap.push(
                    Rect { x: area.x, y: row_y, width: area.width, height: 1 },
                    ClickTarget::SettingsCursor { idx: i },
                );
            }

            lines.push(Line::from(vec![
                Span::raw(marker),
                Span::styled(status_sym, Style::default().fg(status_col)),
                Span::raw(" "),
                Span::styled(store.name.as_str(), name_style),
                Span::raw("  "),
                Span::styled(url_short, Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    f.render_stateful_widget(Paragraph::new(lines), area, &mut ParagraphState::new());
}

fn render_store_modules(f: &mut RenderCtx<'_>, state: &AppState, area: Rect, _cmap: &mut ClickMap) {
    let focused = state.settings_focus == SettingsFocus::Content
        && state.settings_section == SettingsSection::Store
        && state.settings_store_focus == StoreSettingsFocus::Modules;

    let pending_count = state.settings_module_pending.len();

    // Header + footer hint
    let header = Line::from(vec![
        Span::styled(
            state.t("settings.store.tab.modules"),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        ),
    ]);

    let mut lines: Vec<Line<'_>> = vec![header];

    // Only show entries from enabled stores (or bundled entries with no source).
    let enabled_store_names: std::collections::HashSet<&str> = state.settings.stores
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.name.as_str())
        .collect();
    let filtered: Vec<(usize, &fsn_core::store::StoreEntry)> = state.store_entries.iter().enumerate()
        .filter(|(_, e)| e.store_source.is_empty() || enabled_store_names.contains(e.store_source.as_str()))
        .collect();

    if filtered.is_empty() && state.store_entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "Loading store index…",
            Style::default().fg(Color::DarkGray),
        )));
    } else if filtered.is_empty() {
        lines.push(Line::from(Span::styled(
            "No modules from enabled stores.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, (_, entry)) in filtered.iter().enumerate() {
            let is_installed = state.settings.is_installed(&entry.id);
            let is_pending   = state.settings_module_pending.contains(&entry.id);
            let is_sel       = focused && i == state.settings_module_cursor;

            let checkbox = match (is_installed, is_pending) {
                (true,  true)  => "[ ]", // pending uninstall
                (true,  false) => "[x]",
                (false, true)  => "[x]", // pending install
                (false, false) => "[ ]",
            };
            let chk_col    = if is_installed || is_pending { Color::Green } else { Color::DarkGray };
            let marker     = if is_sel { "▶ " } else { "  " };
            let name_style = if is_sel {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if is_installed {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let status_badge: Vec<Span<'_>> = if is_pending {
                vec![Span::styled("  *", Style::default().fg(Color::Yellow))]
            } else if is_installed {
                vec![Span::styled("  ✓", Style::default().fg(Color::Green))]
            } else {
                vec![]
            };

            let mut spans = vec![
                Span::raw(marker),
                Span::styled(checkbox, Style::default().fg(chk_col)),
                Span::raw(" "),
                Span::styled(entry.id.as_str(), name_style),
            ];
            spans.extend(status_badge);
            lines.push(Line::from(spans));
        }
    }

    // ── Language Packs section ────────────────────────────────────────────────
    //
    // Uses "lang:{code}" prefix in settings_module_pending to distinguish
    // language toggles from module toggles (same pending set, different prefix).
    //
    // Cursor indices: modules occupy 0..n_modules, langs occupy n_modules..
    let n_modules = filtered.len();

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "── Language Packs ────────────────",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
    ]));

    if state.store_langs.is_empty() {
        lines.push(Line::from(Span::styled(
            "  Loading language index…",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (j, entry) in state.store_langs.iter().enumerate() {
            let cursor_idx  = n_modules + j;
            let is_installed = state.available_langs.iter().any(|d| d.code == entry.code);
            let pending_key  = format!("lang:{}", entry.code);
            let is_pending   = state.settings_module_pending.contains(&pending_key);
            let is_sel       = focused && state.settings_module_cursor == cursor_idx;

            let checkbox = match (is_installed, is_pending) {
                (true,  true)  => "[ ]", // pending remove
                (true,  false) => "[x]",
                (false, true)  => "[x]", // pending download
                (false, false) => "[ ]",
            };
            let chk_col    = if is_installed || is_pending { Color::Green } else { Color::DarkGray };
            let marker     = if is_sel { "▶ " } else { "  " };
            let code_up    = entry.code.to_uppercase();
            let name_style = if is_sel {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if is_installed {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let status_badge: Vec<Span<'_>> = if is_pending {
                vec![Span::styled("  *", Style::default().fg(Color::Yellow))]
            } else if is_installed {
                vec![Span::styled("  ✓ installed", Style::default().fg(Color::Green))]
            } else {
                vec![Span::styled("  ↓ available", Style::default().fg(Color::DarkGray))]
            };

            let mut spans = vec![
                Span::raw(marker),
                Span::styled(checkbox, Style::default().fg(chk_col)),
                Span::raw(" "),
                Span::styled(format!("{:<4}", code_up), Style::default().fg(Color::Yellow)),
                Span::styled(format!("{:<12}", entry.name.as_str()), name_style),
            ];
            spans.extend(status_badge);
            lines.push(Line::from(spans));
        }
    }

    // Pending changes footer — shows module + language counts separately.
    let n_lang_pending = state.settings_module_pending.iter()
        .filter(|k| k.starts_with("lang:"))
        .count();
    let n_mod_pending  = pending_count.saturating_sub(n_lang_pending);
    let total_pending  = pending_count;

    if total_pending > 0 {
        lines.push(Line::from(""));
        let detail = if n_mod_pending > 0 && n_lang_pending > 0 {
            format!("  ({} modules, {} languages)", n_mod_pending, n_lang_pending)
        } else if n_mod_pending > 0 {
            format!("  ({} modules)", n_mod_pending)
        } else {
            format!("  ({} languages)", n_lang_pending)
        };
        lines.push(Line::from(vec![
            Span::styled(
                state.t("settings.store.hint.apply"),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(detail, Style::default().fg(Color::DarkGray)),
        ]));
    }

    f.render_stateful_widget(Paragraph::new(lines), area, &mut ParagraphState::new());
}

// ── Languages content ─────────────────────────────────────────────────────────
//
// Design: Unified checkbox list — one row per language, installed = [x], not installed = [ ].
//
// Cursor layout:
//   0              → English (built-in, always [x])
//   1..store_langs → each store language in Store index order
//
// When store_langs is empty (still fetching): fall back to showing only installed langs.
//
// Interactions:
//   Enter  → activate language for UI (only if installed)
//   Space  → toggle: download if [ ], remove if [x]
//   Del/D  → remove installed language
//   ←/Esc  → back to sidebar

fn render_languages(f: &mut RenderCtx<'_>, state: &AppState, area: Rect, cmap: &mut ClickMap) {
    let ui_lang   = state.lang;
    let focused   = state.settings_focus == SettingsFocus::Content
        && state.settings_section == SettingsSection::Languages;
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut line_y = area.y;

    // ── English — built-in, always index 0 ────────────────────────────────────
    {
        let is_active = matches!(state.lang, crate::app::Lang::En);
        let is_sel    = focused && state.lang_cursor == 0;
        if line_y < area.bottom() {
            cmap.push(
                Rect { x: area.x, y: line_y, width: area.width, height: 1 },
                ClickTarget::LangCursor { idx: 0 },
            );
        }
        line_y += 1;
        lines.push(lang_checkbox_row(
            "[x]".to_string(), "EN".to_string(), "English".to_string(),
            is_active, is_sel, true,
            t(ui_lang, "settings.lang.builtin").to_string(),
        ));
    }

    if state.store_langs.is_empty() {
        // Store index not yet fetched — show installed languages only.
        lines.push(Line::from(""));
        line_y += 1;
        for (i, dl) in state.available_langs.iter().enumerate() {
            let cursor_idx = i + 1;
            let is_active  = matches!(state.lang, crate::app::Lang::Dynamic(d) if d.code == dl.code);
            let is_sel     = focused && state.lang_cursor == cursor_idx;
            if line_y < area.bottom() {
                cmap.push(
                    Rect { x: area.x, y: line_y, width: area.width, height: 1 },
                    ClickTarget::LangCursor { idx: cursor_idx },
                );
            }
            line_y += 1;
            let (api_label, _) = if dl.api_version == TRANSLATION_API_VERSION {
                (t(ui_lang, "settings.lang.api_ok"), Color::Green)
            } else {
                (t(ui_lang, "settings.lang.api_warn"), Color::Yellow)
            };
            let info = format!("{}%  {}", dl.completeness, api_label);
            lines.push(lang_checkbox_row(
                "[x]".to_string(), dl.code_upper.to_string(), dl.name.to_string(),
                is_active, is_sel, true, info,
            ));
        }
        if state.available_langs.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "Loading Store index…",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    } else {
        // Store index loaded — show all languages as a unified checkbox list.
        lines.push(Line::from(""));
        line_y += 1;

        for (i, entry) in state.store_langs.iter().enumerate() {
            let cursor_idx  = i + 1;
            let is_installed = state.available_langs.iter().any(|d| d.code == entry.code);
            let is_active    = matches!(state.lang, crate::app::Lang::Dynamic(d) if d.code == entry.code);
            let is_sel       = focused && state.lang_cursor == cursor_idx;

            if line_y < area.bottom() {
                cmap.push(
                    Rect { x: area.x, y: line_y, width: area.width, height: 1 },
                    ClickTarget::LangCursor { idx: cursor_idx },
                );
            }
            line_y += 1;

            let checkbox  = if is_installed { "[x]" } else { "[ ]" };
            let code_up   = entry.code.to_uppercase();
            let info_str  = if is_installed {
                format!("{}%  ✓", entry.completeness)
            } else {
                format!("{}%  ↓ Space", entry.completeness)
            };
            lines.push(lang_checkbox_row(
                checkbox.to_string(), code_up, entry.name.clone(),
                is_active, is_sel, is_installed, info_str,
            ));
        }
    }

    let _ = line_y;
    f.render_stateful_widget(Paragraph::new(lines), area, &mut ParagraphState::new());
}

/// Build one language row for the checkbox list.
///
///   ▶ [x] [DE] Deutsch    ✓ Active   100%  ✓
///     [ ] [FR] Français              100%  ↓ Space
fn lang_checkbox_row(
    checkbox:     String,
    code_upper:   String,
    name:         String,
    is_active:    bool,
    is_sel:       bool,
    is_installed: bool,
    info:         String,
) -> Line<'static> {
    let marker     = if is_sel { "▶ " } else { "  " };
    let chk_col    = if is_installed { Color::Green  } else { Color::DarkGray };
    let name_style = if is_sel {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else if is_installed {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let active_badge: Vec<Span<'static>> = if is_active {
        vec![
            Span::raw("  "),
            Span::styled("✓ Active", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]
    } else {
        vec![]
    };
    let mut spans = vec![
        Span::raw(marker),
        Span::styled(checkbox,             Style::default().fg(chk_col)),
        Span::raw(" "),
        Span::styled(format!("[{code_upper}] "), Style::default().fg(Color::Yellow)),
        Span::styled(name,                  name_style),
    ];
    spans.extend(active_badge);
    spans.push(Span::raw("  "));
    spans.push(Span::styled(info, Style::default().fg(Color::DarkGray)));
    Line::from(spans)
}

// ── General content ───────────────────────────────────────────────────────────

fn render_general(f: &mut RenderCtx<'_>, _state: &AppState, area: Rect) {
    // Placeholder — future: theme, log level, auto-update, etc.
    f.render_stateful_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("Coming soon", Style::default().fg(Color::DarkGray)),
            ]),
        ]),
        area,
        &mut ParagraphState::new(),
    );
}

// ── About content ─────────────────────────────────────────────────────────────

fn render_about(f: &mut RenderCtx<'_>, _state: &AppState, area: Rect) {
    let build_time = crate::BUILD_TIME;
    let git_hash   = crate::GIT_HASH;
    let version    = env!("CARGO_PKG_VERSION");

    let lines: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("FreeSynergy", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(".Node",       Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Version:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("v{version}"), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Build:      ", Style::default().fg(Color::DarkGray)),
            Span::styled(build_time,    Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Commit:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(git_hash,      Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("License:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("MIT",         Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Website:    ", Style::default().fg(Color::DarkGray)),
            Span::styled("freesynergy.net", Style::default().fg(Color::Cyan)),
        ]),
    ];

    f.render_stateful_widget(Paragraph::new(lines), area, &mut ParagraphState::new());
}

// ── Hint bar ──────────────────────────────────────────────────────────────────

fn render_hint(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let key = match state.settings_focus {
        SettingsFocus::Sidebar => "settings.hint.sidebar",
        SettingsFocus::Content => match state.settings_section {
            SettingsSection::General   => "settings.hint.general",
            SettingsSection::Store     => "settings.hint.stores",
            SettingsSection::Languages => "settings.hint.languages",
            SettingsSection::About     => "settings.hint.about",
        },
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
