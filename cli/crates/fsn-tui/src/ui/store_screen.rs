// Store browse screen.
//
// Design Pattern: Composite — sidebar (type categories + package list) +
//   detail panel (full package metadata).
//
// Single source of truth: reads directly from AppState::store_entries
//   and AppState::settings.installed_modules — no local copy.
//
// Layout:
//   Left (30%): Type category header + packages list
//   Right (70%): Package detail or type description

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use fsn_core::store::StoreEntry;

use crate::app::{AppState, StoreScreenFocus};
use crate::ui::components::{Component, FooterBar, HeaderBar};
use crate::ui::layout::{AppLayout, LayoutConfig};
use crate::ui::render_ctx::RenderCtx;

// ── Category grouping ─────────────────────────────────────────────────────────

/// A flat list item in the store sidebar — either a category header or a package row.
#[derive(Debug)]
enum SidebarRow<'a> {
    Category(&'static str),
    Package(&'a StoreEntry),
}

/// Build the ordered sidebar row list, grouped by primary ServiceType category.
fn build_sidebar_rows(entries: &[StoreEntry]) -> Vec<SidebarRow<'_>> {
    // Category ordering — matches ServiceType::category() return values.
    const CATEGORY_ORDER: &[&str] = &[
        "proxy", "iam", "communication", "developer",
        "knowledge", "project", "geo", "monitoring",
        "infrastructure", "automation", "custom",
    ];

    let mut rows: Vec<SidebarRow<'_>> = Vec::new();

    for &cat in CATEGORY_ORDER {
        let cat_entries: Vec<&StoreEntry> = entries.iter()
            .filter(|e| e.primary_type().category() == cat)
            .collect();
        if cat_entries.is_empty() { continue; }
        rows.push(SidebarRow::Category(cat));
        for entry in cat_entries {
            rows.push(SidebarRow::Package(entry));
        }
    }

    // Entries whose category is not in CATEGORY_ORDER (should not happen normally).
    let covered_ids: std::collections::HashSet<&str> = rows.iter()
        .filter_map(|r| if let SidebarRow::Package(e) = r { Some(e.id.as_str()) } else { None })
        .collect();
    let uncategorized: Vec<&StoreEntry> = entries.iter()
        .filter(|e| !covered_ids.contains(e.id.as_str()))
        .collect();
    if !uncategorized.is_empty() {
        rows.push(SidebarRow::Category("other"));
        for entry in uncategorized {
            rows.push(SidebarRow::Package(entry));
        }
    }

    rows
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn render(f: &mut RenderCtx<'_>, state: &mut AppState) {
    let full = f.area();
    state.click_map.clear();

    let layout = AppLayout::compute(full, &LayoutConfig {
        topbar_height: 5,
        left_width: Some(32),
        ..LayoutConfig::default()
    });

    HeaderBar.render(f, layout.topbar, state);
    FooterBar.render(f, layout.footer_primary, state);

    let sidebar_area = layout.body.left.unwrap_or(layout.body.main);
    let detail_area  = layout.body.main;

    render_sidebar(f, state, sidebar_area);
    render_detail(f, state, detail_area);
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

fn render_sidebar(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let focused    = state.store_screen_focus == StoreScreenFocus::Sidebar;
    let cursor     = state.store_cursor;

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
        x:     area.x + 1,
        y:     area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };

    // Filter to only entries from enabled stores (or bundled entries with no source).
    let enabled_store_names: std::collections::HashSet<&str> = state.settings.stores
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.name.as_str())
        .collect();
    let visible_entries: Vec<&fsn_core::store::StoreEntry> = state.store_entries.iter()
        .filter(|e| e.store_source.is_empty() || enabled_store_names.contains(e.store_source.as_str()))
        .collect();

    if visible_entries.is_empty() {
        f.render_stateful_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Loading store…",
                    Style::default().fg(Color::DarkGray),
                )),
            ]),
            inner,
            &mut ParagraphState::new(),
        );
        return;
    }

    // Build sidebar rows from the owned copies of filtered entries.
    let owned_entries: Vec<fsn_core::store::StoreEntry> = visible_entries.iter().map(|e| (*e).clone()).collect();
    let rows = build_sidebar_rows(&owned_entries);
    let mut lines: Vec<Line<'_>> = Vec::new();

    // Count only package rows for cursor tracking.
    let mut pkg_idx = 0usize;

    for row in &rows {
        match row {
            SidebarRow::Category(cat) => {
                let label = format!("── {} ──", cat.to_uppercase());
                lines.push(Line::from(Span::styled(
                    label,
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
                )));
            }
            SidebarRow::Package(entry) => {
                let is_sel       = focused && pkg_idx == cursor;
                let is_installed = state.settings.is_installed(&entry.id);
                let checkbox     = if is_installed { "[x]" } else { "[ ]" };
                let chk_col      = if is_installed { Color::Green } else { Color::DarkGray };
                let marker       = if is_sel { "▶ " } else { "  " };
                let name_style   = if is_sel {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else if is_installed {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                lines.push(Line::from(vec![
                    Span::raw(marker),
                    Span::styled(checkbox, Style::default().fg(chk_col)),
                    Span::raw(" "),
                    Span::styled(entry.name.as_str(), name_style),
                ]));
                pkg_idx += 1;
            }
        }
    }

    f.render_stateful_widget(Paragraph::new(lines), inner, &mut ParagraphState::new());
}

// ── Detail panel ─────────────────────────────────────────────────────────────

fn render_detail(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Filter entries from enabled stores (same logic as render_sidebar).
    let enabled_store_names_detail: std::collections::HashSet<&str> = state.settings.stores
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.name.as_str())
        .collect();
    let packages: Vec<&StoreEntry> = state.store_entries.iter()
        .filter(|e| e.store_source.is_empty() || enabled_store_names_detail.contains(e.store_source.as_str()))
        .collect();

    if packages.is_empty() {
        f.render_stateful_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    state.t("store.select_package"),
                    Style::default().fg(Color::DarkGray),
                )),
            ]),
            inner,
            &mut ParagraphState::new(),
        );
        return;
    }

    // Resolve the currently selected package (by cursor index over package rows only).
    let cursor = state.store_cursor.min(packages.len().saturating_sub(1));

    if let Some(entry) = packages.get(cursor) {
        render_package_detail(f, state, inner, entry);
    } else {
        f.render_stateful_widget(
            Paragraph::new(Line::from(Span::styled(
                state.t("store.select_package"),
                Style::default().fg(Color::DarkGray),
            ))),
            inner,
            &mut ParagraphState::new(),
        );
    }
}

fn render_package_detail(
    f:     &mut RenderCtx<'_>,
    state: &AppState,
    area:  Rect,
    entry: &StoreEntry,
) {
    let is_installed    = state.settings.is_installed(&entry.id);
    let type_label: String = entry.service_types.iter()
        .map(|t| t.label())
        .collect::<Vec<_>>()
        .join(" / ");

    let status_text = if is_installed {
        state.t("store.status.installed")
    } else {
        state.t("store.status.not_installed")
    };
    let status_col = if is_installed { Color::Green } else { Color::DarkGray };

    // Build detail lines.
    let mut lines: Vec<Line<'_>> = Vec::new();

    // Name + version header
    lines.push(Line::from(vec![
        Span::styled(entry.name.as_str(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(format!("v{}", entry.version), Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(type_label, Style::default().fg(Color::Cyan)),
    ]));

    lines.push(Line::from(""));

    // Status
    lines.push(Line::from(vec![
        Span::styled("Status:   ", Style::default().fg(Color::DarkGray)),
        Span::styled(status_text, Style::default().fg(status_col).add_modifier(Modifier::BOLD)),
    ]));

    // ID
    lines.push(Line::from(vec![
        Span::styled("ID:       ", Style::default().fg(Color::DarkGray)),
        Span::styled(entry.id.as_str(), Style::default().fg(Color::White)),
    ]));

    // Author
    if let Some(ref author) = entry.author {
        lines.push(Line::from(vec![
            Span::styled("Author:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(author.as_str(), Style::default().fg(Color::White)),
        ]));
    }

    // License
    if let Some(ref license) = entry.license {
        lines.push(Line::from(vec![
            Span::styled("License:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(license.as_str(), Style::default().fg(Color::White)),
        ]));
    }

    // Website
    if let Some(ref website) = entry.website {
        lines.push(Line::from(vec![
            Span::styled("Website:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(website.as_str(), Style::default().fg(Color::Cyan)),
        ]));
    }

    // Tags
    if !entry.tags.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Tags:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(entry.tags.join(", "), Style::default().fg(Color::DarkGray)),
        ]));
    }

    // Dates
    if let Some(ref created) = entry.created_at {
        lines.push(Line::from(vec![
            Span::styled("Created:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(created.as_str(), Style::default().fg(Color::DarkGray)),
        ]));
    }
    if let Some(ref updated) = entry.updated_at {
        lines.push(Line::from(vec![
            Span::styled("Updated:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(updated.as_str(), Style::default().fg(Color::DarkGray)),
        ]));
    }

    // Min version
    if let Some(ref min_ver) = entry.min_fsn_version {
        lines.push(Line::from(vec![
            Span::styled("Requires: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("FSN >= {min_ver}"), Style::default().fg(Color::DarkGray)),
        ]));
    }

    lines.push(Line::from(""));

    // Description (word-wrapped to available width)
    lines.push(Line::from(Span::styled(
        "Description",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )));
    let desc_width = (area.width as usize).saturating_sub(2);
    for chunk in word_wrap(&entry.description, desc_width) {
        lines.push(Line::from(Span::styled(chunk, Style::default().fg(Color::White))));
    }

    // What it provides
    let first_type = entry.primary_type();
    let provides   = first_type.what_it_provides();
    if !provides.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Provides",
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )));
        for p in provides {
            lines.push(Line::from(vec![
                Span::raw("  • "),
                Span::styled(*p, Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    // Action hints
    lines.push(Line::from(""));
    let hint_line = if is_installed {
        Line::from(vec![
            Span::styled(state.t("store.hint.uninstall"), Style::default().fg(Color::Yellow)),
            Span::raw("  "),
            Span::styled(state.t("store.hint.reinstall"), Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled(state.t("store.hint.install"), Style::default().fg(Color::Green)),
        ])
    };
    lines.push(hint_line);

    // Apply scroll offset.
    let scroll = state.store_detail_scroll as usize;
    let visible: Vec<Line<'_>> = lines.into_iter().skip(scroll).collect();

    f.render_stateful_widget(Paragraph::new(visible), area, &mut ParagraphState::new());
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Simple word-wrap: splits `text` into lines of at most `width` characters.
fn word_wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 { return vec![text.to_string()]; }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current.clone());
            current = word.to_string();
        }
    }
    if !current.is_empty() { lines.push(current); }
    if lines.is_empty() { lines.push(String::new()); }
    lines
}
