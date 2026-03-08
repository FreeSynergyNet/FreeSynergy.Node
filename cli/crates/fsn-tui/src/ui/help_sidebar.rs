// Contextual help sidebar — toggled with F1.
//
// Design: screen-centric (Option B).
// - `build_help()` returns sections based on current screen + focused field key
// - All text lives in i18n.rs (static) or i18n::field_help() (field-specific)
// - No FormNode changes required — uses existing node.key() as lookup index
//
// Layout: fixed 30-column right panel (horizontal split in ui/mod.rs).
// F1 toggles `AppState::help_visible`.
// Esc when help open → closes help (priority over screen-specific Esc).

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{Lang, ResourceKind, Screen};

// ── Width ─────────────────────────────────────────────────────────────────────

/// Fixed sidebar width in columns.
pub const SIDEBAR_WIDTH: u16 = 30;

// ── Data types ────────────────────────────────────────────────────────────────

/// One key–description pair shown in the sidebar.
pub struct HelpEntry {
    /// Displayed key label, e.g. "Tab", "Ctrl+Q". Empty string for text-only entries.
    pub key:  &'static str,
    /// Translated description.
    pub desc: &'static str,
}

/// A titled group of help entries shown in the sidebar.
pub struct HelpSection {
    pub title:   &'static str,
    pub entries: Vec<HelpEntry>,
}

// ── Build ─────────────────────────────────────────────────────────────────────

/// Build contextual help sections for the current screen state.
///
/// `focused_key` — the `FormNode::key()` of the currently focused field, if any.
/// `kind`        — active `ResourceKind` when on the form screen.
pub fn build_help(
    screen:      Screen,
    kind:        Option<ResourceKind>,
    focused_key: Option<&'static str>,
    lang:        Lang,
) -> Vec<HelpSection> {
    let t = |k| crate::i18n::t(lang, k);

    match screen {
        Screen::Welcome => vec![
            HelpSection {
                title: t("help.nav"),
                entries: vec![
                    HelpEntry { key: "↑↓",    desc: t("help.nav.select") },
                    HelpEntry { key: "Enter",  desc: t("help.nav.open") },
                    HelpEntry { key: "N",      desc: t("help.new_project") },
                    HelpEntry { key: "L",      desc: t("help.lang") },
                    HelpEntry { key: "Ctrl+Q", desc: t("help.quit") },
                ],
            },
        ],

        Screen::Dashboard => vec![
            HelpSection {
                title: t("help.nav"),
                entries: vec![
                    HelpEntry { key: "↑↓",    desc: t("help.nav.select") },
                    HelpEntry { key: "Tab",    desc: t("help.nav.panel") },
                    HelpEntry { key: "Enter",  desc: t("help.nav.open") },
                    HelpEntry { key: "D",      desc: t("help.deploy") },
                    HelpEntry { key: "E",      desc: t("help.export") },
                    HelpEntry { key: "Del",    desc: t("help.delete") },
                    HelpEntry { key: "L",      desc: t("help.lang") },
                    HelpEntry { key: "Ctrl+Q", desc: t("help.quit") },
                ],
            },
        ],

        Screen::NewProject => {
            let form_title = match kind {
                Some(ResourceKind::Host)    => t("help.form.host"),
                Some(ResourceKind::Service) => t("help.form.service"),
                Some(ResourceKind::Bot)     => t("help.form.bot"),
                _                           => t("help.form.project"),
            };

            let mut sections = vec![HelpSection {
                title: form_title,
                entries: vec![
                    HelpEntry { key: "Tab",       desc: t("help.form.next") },
                    HelpEntry { key: "Shift+Tab", desc: t("help.form.prev") },
                    HelpEntry { key: "↑↓",        desc: t("help.form.select") },
                    HelpEntry { key: "Enter",     desc: t("help.form.advance") },
                    HelpEntry { key: "^Enter",    desc: t("help.form.submit") },
                    HelpEntry { key: "Ctrl+→",    desc: t("help.form.tab_next") },
                    HelpEntry { key: "Ctrl+←",    desc: t("help.form.tab_prev") },
                    HelpEntry { key: "Esc",       desc: t("help.form.cancel") },
                ],
            }];

            // Field-specific hint — only when there is content for this key
            if let Some(key) = focused_key {
                if let Some(hint) = crate::i18n::field_help(lang, key) {
                    sections.push(HelpSection {
                        title: t("help.field"),
                        entries: vec![HelpEntry { key: "", desc: hint }],
                    });
                }
            }

            sections
        }

        Screen::TaskWizard => vec![
            HelpSection {
                title: t("help.nav"),
                entries: vec![
                    HelpEntry { key: "Tab",       desc: t("help.form.next") },
                    HelpEntry { key: "^Enter",     desc: t("help.form.submit") },
                    HelpEntry { key: "^←/^→",     desc: t("help.form.tab_next") },
                    HelpEntry { key: "Esc",        desc: t("help.form.cancel") },
                ],
            },
        ],
    }
}

// ── Render ────────────────────────────────────────────────────────────────────

/// Render the help sidebar into `area`.
///
/// Called after the main content is rendered so the sidebar appears to the right.
pub fn render_help_sidebar(f: &mut Frame, area: Rect, sections: &[HelpSection], lang: Lang) {
    let title = format!(" {} ", crate::i18n::t(lang, "help.title"));

    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Build content lines
    let mut lines: Vec<Line> = vec![];
    for section in sections {
        // Section title
        lines.push(Line::from(Span::styled(
            section.title,
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));

        for entry in &section.entries {
            if entry.key.is_empty() {
                // Text-only entry (field hint) — wrap manually
                let desc = entry.desc;
                let max_w = inner.width.saturating_sub(2) as usize;
                let mut remaining = desc;
                while !remaining.is_empty() {
                    let cut = if remaining.len() <= max_w {
                        remaining.len()
                    } else {
                        remaining[..max_w].rfind(' ').unwrap_or(max_w)
                    };
                    let (line_text, rest) = remaining.split_at(cut);
                    lines.push(Line::from(Span::styled(
                        format!(" {}", line_text.trim()),
                        Style::default().fg(Color::White),
                    )));
                    remaining = rest.trim_start();
                }
            } else {
                // Key + description
                let key_w = 10usize;
                let key_col = format!("{:<w$}", entry.key, w = key_w);
                lines.push(Line::from(vec![
                    Span::styled(" ", Style::default()),
                    Span::styled(key_col, Style::default().fg(Color::Cyan)),
                    Span::styled(entry.desc, Style::default().fg(Color::White)),
                ]));
            }
        }

        // Gap between sections
        lines.push(Line::from(""));
    }

    // Footer hint
    if !lines.is_empty() { lines.push(Line::from("".to_string())); }
    lines.push(Line::from(Span::styled(
        crate::i18n::t(lang, "help.close_hint"),
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left),
        inner,
    );
}
