// Generic resource editor screen.
//
// Renders any `ResourceForm`. Each field node renders itself via `node.render()`.
// After all nodes are rendered, the focused node gets `render_overlay()` so
// that dropdowns appear on top of other fields — no special-casing needed here.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
};

use crate::ui::render_ctx::RenderCtx;

use crate::app::{AppState, ResourceForm};
use crate::resource_form::FormErrorKind;
use crate::ui::widgets;

pub fn render(f: &mut RenderCtx<'_>, state: &mut AppState, area: Rect) {
    let Some(ref mut form) = state.current_form else { return };

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(3), // tab bar
            Constraint::Min(1),    // form fields
            Constraint::Length(1), // error line
            Constraint::Length(1), // hint bar
        ])
        .split(area);

    render_header(f, state.lang, form, outer[0]);
    render_tabs(f, state.lang, form, outer[1]);

    // Build inner area with horizontal padding (5% each side)
    let padding = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(5), Constraint::Percentage(90), Constraint::Percentage(5)])
        .split(outer[2]);
    let inner = padding[1];

    render_fields(f, form, inner, state.lang);
    render_error(f, state.lang, form, outer[3]);
    render_hint(f, state, outer[4]);
}

// ── Header ────────────────────────────────────────────────────────────────────

fn render_header(f: &mut RenderCtx<'_>, lang: crate::app::Lang, form: &ResourceForm, area: Rect) {
    let title_key = form.title_key();

    let title = Line::from(vec![
        Span::styled(" FreeSynergy.Node ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("– ", Style::default().fg(Color::DarkGray)),
        Span::styled(crate::i18n::t(lang, title_key),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]);
    let header = Paragraph::new(title)
        .block(Block::default().borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(header, area);

    // Language button top-right
    let lang_area = Rect { x: area.right().saturating_sub(6), y: area.y + 1, width: 4, height: 1 };
    f.render_widget(
        Paragraph::new(Line::from(widgets::lang_button_raw(lang))),
        lang_area,
    );
}

// ── Tab bar ───────────────────────────────────────────────────────────────────

pub(crate) fn render_tabs(f: &mut RenderCtx<'_>, lang: crate::app::Lang, form: &ResourceForm, area: Rect) {
    let tab_titles: Vec<Line> = form.tab_keys.iter().enumerate().map(|(i, &key)| {
        let label       = crate::i18n::t(lang, key);
        let has_missing = form.tab_missing_count(i) > 0;
        let is_active   = i == form.active_tab;

        let text = if has_missing && !is_active {
            format!(" {} ⚠ ", label)
        } else {
            format!(" {} ", label)
        };

        if is_active {
            Line::from(Span::styled(text,
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)))
        } else if has_missing {
            Line::from(Span::styled(text, Style::default().fg(Color::Yellow)))
        } else {
            Line::from(Span::styled(text, Style::default().fg(Color::DarkGray)))
        }
    }).collect();

    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)))
        .select(form.active_tab)
        .divider(Span::styled("  ", Style::default()));
    f.render_widget(tabs, area);
}

// ── Form fields ───────────────────────────────────────────────────────────────

pub(crate) fn render_fields(f: &mut RenderCtx<'_>, form: &mut ResourceForm, inner: Rect, lang: crate::app::Lang) {
    let tab_indices = form.current_tab_indices();
    

    let mut y = inner.y;
    let mut overlay_slot: Option<usize> = None; // which slot needs render_overlay

    for (slot, &node_idx) in tab_indices.iter().enumerate() {
        let h = form.nodes[node_idx].preferred_height();
        if y + h > inner.bottom() { break; }
        let field_rect = Rect { x: inner.x, y, width: inner.width, height: h };
        y += h;

        let focused = form.active_field == slot;
        form.nodes[node_idx].render(f, field_rect, focused, lang);

        if focused { overlay_slot = Some(slot); }
    }

    // Submit button on the last tab
    if form.is_last_tab() {
        let btn_y = y + 1;
        if btn_y + 3 <= inner.bottom() {
            let btn_area = Rect { x: inner.x, y: btn_y, width: inner.width / 3, height: 3 };
            let missing  = form.missing_required();
            let disabled = !missing.is_empty();
            let submit_key = if form.edit_id.is_some() { "form.submit.edit" } else { form.kind.submit_key() };
            let btn = Paragraph::new(widgets::button_line(crate::i18n::t(lang, submit_key), true, disabled))
                .block(Block::default().borders(Borders::ALL).border_style(
                    if disabled { Style::default().fg(Color::DarkGray) }
                    else        { Style::default().fg(Color::Green) }
                ))
                .alignment(Alignment::Center);
            f.render_widget(btn, btn_area);
        }
    }

    // Dropdown overlay rendered LAST so it appears on top of other fields
    if let Some(slot) = overlay_slot {
        if let Some(&node_idx) = tab_indices.get(slot) {
            form.nodes[node_idx].render_overlay(f, inner, lang);
        }
    }
}

// ── Error line ────────────────────────────────────────────────────────────────

pub(crate) fn render_error(f: &mut RenderCtx<'_>, lang: crate::app::Lang, form: &ResourceForm, area: Rect) {
    if let Some(ref err) = form.error {
        let (icon, color) = match form.error_kind {
            FormErrorKind::Validation => ("⚠ ", Color::Yellow),
            FormErrorKind::IoError    => ("✗ ", Color::Red),
        };
        let line = Line::from(vec![
            Span::styled(
                format!("  {}", icon),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(err.as_str(), Style::default().fg(color)),
        ]);
        f.render_widget(Paragraph::new(line), area);
    } else if form.touched {
        // Live validation hint: show remaining required fields count.
        let missing = form.missing_required();
        if missing.is_empty() {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!("  {}", crate::i18n::t(lang, "form.all_required_filled")),
                    Style::default().fg(Color::Green),
                ))),
                area,
            );
        } else {
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("  ⚠ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!("{} {}", missing.len(), crate::i18n::t(lang, "form.missing_required")),
                        Style::default().fg(Color::Yellow),
                    ),
                ])),
                area,
            );
        }
    } else {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                crate::i18n::t(lang, "form.required"),
                Style::default().fg(Color::DarkGray),
            ))),
            area,
        );
    }
}

// ── Hint bar ──────────────────────────────────────────────────────────────────

fn render_hint(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let key = if state.ctrl_hint { "form.hint.ctrl" } else { "form.hint" };
    let hint_text = state.t(key);
    let _f1_label = state.t("help.title");

    let line = Line::from(vec![
        Span::styled(hint_text, Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(
            "F1=Hilfe",
            Style::default().fg(if state.help_visible { Color::Cyan } else { Color::DarkGray }),
        ),
    ]);
    f.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
}
