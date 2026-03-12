// Generic resource editor screen.
//
// Renders any `ResourceForm` from the active `FormQueue` tab.
// Each field node renders itself via `node.render()`.
// After all nodes are rendered every node gets `render_overlay()` — dropdowns
// appear on top of other fields without any special-casing here.
//
// Layout (top to bottom):
//   1. Header bar (3 rows) — title + lang button
//   2. Queue tab bar (2 rows, only when FormQueue has > 1 tab)
//   3. Form tab bar (3 rows, only when form has > 1 section tab)
//   4. Form fields (remaining space)
//   5. Error/validation line (1 row)
//   6. Hint bar (1 row)
//
// Mouse registration (ClickMap):
//   render_header     → ClickTarget::LangToggle
//   render_queue_bar  → ClickTarget::QueueTab { idx }
//   render_fields     → ClickTarget::FormField (one per visible field)
//                       ClickTarget::FormSubmit (when on last tab)
//
// Design Pattern: Composite — each visible section (header, queue bar, form
// tabs, fields, error, hint) is a pure fn with its own responsibilities.
// No section function knows about the others.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use crate::click_map::{ClickMap, ClickTarget};
use crate::ui::render_ctx::RenderCtx;

use crate::app::{AppState, ResourceForm};
use crate::form_queue::FormQueue;
use crate::resource_form::FormErrorKind;
use crate::ui::widgets;

pub fn render(f: &mut RenderCtx<'_>, state: &mut AppState, area: Rect) {
    if state.form_queue.is_none() { return; }

    let has_queue_bar = state.form_queue.as_ref().unwrap().has_multiple();
    let tab_bar_h     = if state.form_queue.as_ref().unwrap().active_form().tab_keys.len() > 1 { 3 } else { 0 };
    let queue_bar_h: u16 = if has_queue_bar { 2 } else { 0 };

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),              // header
            Constraint::Length(queue_bar_h),    // queue tab bar (hidden when single form)
            Constraint::Length(tab_bar_h),      // form tab bar (hidden when single section)
            Constraint::Min(1),                 // form fields
            Constraint::Length(1),              // error line
            Constraint::Length(1),              // hint bar
        ])
        .split(area);

    // Take click_map from state — avoids borrow conflict while form is live.
    // Disjoint field borrows: form→state.form_queue, cmap→state.click_map.
    let mut cmap = std::mem::take(&mut state.click_map);
    cmap.clear();

    // ── Header ───────────────────────────────────────────────────────────────
    {
        let form = state.form_queue.as_ref().unwrap().active_form();
        render_header(f, state.lang, form, outer[0], &mut cmap);
    }

    // ── Queue tab bar ─────────────────────────────────────────────────────────
    if has_queue_bar {
        render_queue_bar(f, state.form_queue.as_ref().unwrap(), state.lang, outer[1], &mut cmap);
    }

    // ── Form section tab bar ──────────────────────────────────────────────────
    if tab_bar_h > 0 {
        let form = state.form_queue.as_ref().unwrap().active_form();
        render_tabs(f, state.lang, form, outer[2]);
    }

    // ── Form fields ───────────────────────────────────────────────────────────
    // Build inner area with horizontal padding (5% each side).
    let padding = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(5), Constraint::Percentage(90), Constraint::Percentage(5)])
        .split(outer[3]);
    let inner = padding[1];

    {
        let form = state.form_queue.as_mut().unwrap().active_form_mut();
        render_fields(f, form, inner, state.lang, &mut cmap);
    }

    // Return click_map to state — render_error and render_hint don't need it.
    state.click_map = cmap;

    // ── Error line + hint bar ─────────────────────────────────────────────────
    {
        let form = state.form_queue.as_ref().unwrap().active_form();
        render_error(f, state.lang, form, outer[4]);
    }
    render_hint(f, state, outer[5]);
}

// ── Header ────────────────────────────────────────────────────────────────────

fn render_header(
    f:    &mut RenderCtx<'_>,
    lang: crate::app::Lang,
    form: &ResourceForm,
    area: Rect,
    cmap: &mut ClickMap,
) {
    let title_key = form.title_key();

    let title = Line::from(vec![
        Span::styled(" FreeSynergy.Node ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("– ", Style::default().fg(Color::DarkGray)),
        Span::styled(crate::i18n::t(lang, title_key),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]);
    f.render_stateful_widget(
        Paragraph::new(title)
            .block(Block::default().borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray))),
        area,
        &mut ParagraphState::new(),
    );

    // Language button top-right — render + register in click_map.
    let lang_area = Rect { x: area.right().saturating_sub(6), y: area.y + 1, width: 4, height: 1 };
    f.render_stateful_widget(
        Paragraph::new(Line::from(widgets::lang_button_raw(lang))),
        lang_area,
        &mut ParagraphState::new(),
    );
    cmap.push(lang_area, ClickTarget::LangToggle);
}

// ── Queue tab bar — switches between queued forms ─────────────────────────────
//
// Design Pattern: Registry — each rendered tab is registered in the click_map
// as `QueueTab { idx }` so mouse.rs can switch the active form on click.
//
// Visual convention:
//   ✓ label   = done (green, greyed out)
//   ▶ label   = active (cyan, bold)
//     label   = pending (white)

fn render_queue_bar(
    f:    &mut RenderCtx<'_>,
    queue: &FormQueue,
    lang:  crate::app::Lang,
    area:  Rect,
    cmap:  &mut ClickMap,
) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut x = inner.x;
    for (i, tab) in queue.tabs.iter().enumerate() {
        let is_active = i == queue.active;
        let label_key = tab.kind.as_ref()
            .map(|k| k.label_key())
            .unwrap_or_else(|| tab.form.title_key());
        let label = crate::i18n::t(lang, label_key);

        let (prefix, style) = if is_active {
            ("▶ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        } else if tab.done {
            ("✓ ", Style::default().fg(Color::Green))
        } else {
            ("  ", Style::default().fg(Color::White))
        };

        let text   = format!("{}{}", prefix, label);
        let width  = (text.chars().count() as u16 + 2).min(inner.right().saturating_sub(x));
        if width == 0 { break; }

        let tab_area = Rect { x, y: inner.y, width, height: 1 };
        f.render_stateful_widget(
            Paragraph::new(Line::from(Span::styled(text, style))),
            tab_area,
            &mut ParagraphState::new(),
        );
        cmap.push(tab_area, ClickTarget::QueueTab { idx: i });

        x += width + 2; // 2-space gap between tabs
        if x >= inner.right() { break; }

        // Separator
        if i + 1 < queue.tabs.len() {
            let sep = Rect { x: x.saturating_sub(1), y: inner.y, width: 1, height: 1 };
            f.render_stateful_widget(
                Paragraph::new(Line::from(Span::styled("│", Style::default().fg(Color::DarkGray)))),
                sep,
                &mut ParagraphState::new(),
            );
        }
    }
}

// ── Form section tab bar ──────────────────────────────────────────────────────

pub(crate) fn render_tabs(f: &mut RenderCtx<'_>, lang: crate::app::Lang, form: &ResourceForm, area: Rect) {
    // Replaced ratatui Tabs with manual span-based rendering.
    let mut spans: Vec<Span> = vec![];
    for (i, &key) in form.tab_keys.iter().enumerate() {
        let label       = crate::i18n::t(lang, key);
        let has_missing = form.tab_missing_count(i) > 0;
        let is_active   = i == form.active_tab;
        if i > 0 {
            spans.push(Span::styled("  ", Style::default().fg(Color::DarkGray)));
        }
        let text = if has_missing && !is_active {
            format!(" {} ⚠ ", label)
        } else {
            format!(" {} ", label)
        };
        let span = if is_active {
            Span::styled(text, Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD))
        } else if has_missing {
            Span::styled(text, Style::default().fg(Color::Yellow))
        } else {
            Span::styled(text, Style::default().fg(Color::DarkGray))
        };
        spans.push(span);
    }
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_stateful_widget(
        Paragraph::new(Line::from(spans)),
        inner,
        &mut ParagraphState::new(),
    );
}

// ── Form fields ───────────────────────────────────────────────────────────────

pub(crate) fn render_fields(
    f:    &mut RenderCtx<'_>,
    form: &mut ResourceForm,
    inner: Rect,
    lang: crate::app::Lang,
    cmap: &mut ClickMap,
) {
    use ratatui::layout::{Constraint, Direction, Layout};

    let tab_indices = form.current_tab_indices();

    let mut y = inner.y;

    // ── 12-column row grouping ─────────────────────────────────────────────
    //
    // Strategy: pack consecutive nodes into a row as long as col_span sum ≤ 12
    // and each node's rendered width ≥ min_width. When a node doesn't fit (sum
    // would exceed 12) OR its min_width can't be met, flush the current row and
    // start a new one.
    //
    // A "row" is a slice of (slot, node_idx) pairs sharing one horizontal band.
    // Each band's height = max(preferred_height) of nodes in that row.

    let mut rows: Vec<Vec<(usize, usize)>> = vec![];  // Vec<row>; row = Vec<(slot, node_idx)>
    let mut current_row: Vec<(usize, usize)> = vec![];
    let mut col_sum: u8 = 0;

    for (slot, &node_idx) in tab_indices.iter().enumerate() {
        let node    = &form.nodes[node_idx];
        let span    = node.col_span();
        let min_w   = node.min_width();
        let avail_w = if span < 12 {
            (inner.width as u32 * span as u32 / 12) as u16
        } else {
            inner.width
        };

        // Section nodes and nodes that can't fit the min_width always start a new row.
        let force_new = !node.is_focusable() || min_w > 0 && avail_w < min_w;

        if force_new || col_sum + span > 12 {
            if !current_row.is_empty() {
                rows.push(std::mem::take(&mut current_row));
            }
            col_sum = 0;
        }

        current_row.push((slot, node_idx));
        col_sum += span;

        // Non-focusable (section) nodes always get their own row and flush immediately.
        if force_new {
            rows.push(std::mem::take(&mut current_row));
            col_sum = 0;
        }
    }
    if !current_row.is_empty() { rows.push(current_row); }

    // ── Render each row ────────────────────────────────────────────────────
    for row in &rows {
        // Row height = max preferred_height across all nodes in this row.
        let row_h = row.iter()
            .map(|&(_, ni)| form.nodes[ni].preferred_height())
            .max()
            .unwrap_or(0);

        if y + row_h > inner.bottom() { break; }
        let row_rect = Rect { x: inner.x, y, width: inner.width, height: row_h };
        y += row_h;

        if row.len() == 1 {
            // Fast path — no horizontal split needed.
            let (slot, node_idx) = row[0];
            let focused = form.active_field == slot;
            form.nodes[node_idx].render(f, row_rect, focused, lang);
            // Register in click_map only for focusable nodes.
            if form.nodes[node_idx].is_focusable() {
                cmap.push(row_rect, ClickTarget::FormField { slot, node_idx, rect: row_rect });
            }
        } else {
            // Split the row proportionally by col_span.
            let constraints: Vec<Constraint> = row.iter()
                .map(|&(_, ni)| {
                    let pct = form.nodes[ni].col_span() as u16 * 100 / 12;
                    Constraint::Percentage(pct)
                })
                .collect();
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(constraints)
                .split(row_rect);

            for (i, &(slot, node_idx)) in row.iter().enumerate() {
                let focused = form.active_field == slot;
                form.nodes[node_idx].render(f, cols[i], focused, lang);
                if form.nodes[node_idx].is_focusable() {
                    cmap.push(cols[i], ClickTarget::FormField { slot, node_idx, rect: cols[i] });
                }
            }
        }
    }

    // Submit button on the last tab — render + register in click_map.
    if form.is_last_tab() {
        let btn_y = y + 1;
        if btn_y + 3 <= inner.bottom() {
            let btn_area = Rect { x: inner.x, y: btn_y, width: inner.width / 3, height: 3 };
            let missing  = form.missing_required();
            let disabled = !missing.is_empty();
            let submit_key = if form.edit_id.is_some() { "form.submit.edit" } else { form.kind.submit_key() };
            f.render_stateful_widget(
                Paragraph::new(widgets::button_line(crate::i18n::t(lang, submit_key), true, disabled))
                    .block(Block::default().borders(Borders::ALL).border_style(
                        if disabled { Style::default().fg(Color::DarkGray) }
                        else        { Style::default().fg(Color::Green) }
                    ))
                    .alignment(Alignment::Center),
                btn_area,
                &mut ParagraphState::new(),
            );
            // Only register as clickable when not disabled — clicking a greyed-out
            // button would trigger validation errors which feels unexpected.
            if !disabled {
                cmap.push(btn_area, ClickTarget::FormSubmit);
            }
        }
    }

    // Overlays rendered LAST so they appear on top of all fields.
    // Called for every node — each checks its own is_open state.
    // This keeps popups visible even when focus moves elsewhere.
    for &node_idx in &tab_indices {
        form.nodes[node_idx].render_overlay(f, inner, lang);
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
        f.render_stateful_widget(Paragraph::new(line), area, &mut ParagraphState::new());
    } else if form.touched {
        // Live validation hint: show remaining required fields count.
        let missing = form.missing_required();
        if missing.is_empty() {
            f.render_stateful_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!("  {}", crate::i18n::t(lang, "form.all_required_filled")),
                    Style::default().fg(Color::Green),
                ))),
                area,
                &mut ParagraphState::new(),
            );
        } else {
            f.render_stateful_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("  ⚠ ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        format!("{} {}", missing.len(), crate::i18n::t(lang, "form.missing_required")),
                        Style::default().fg(Color::Yellow),
                    ),
                ])),
                area,
                &mut ParagraphState::new(),
            );
        }
    } else {
        f.render_stateful_widget(
            Paragraph::new(Line::from(Span::styled(
                crate::i18n::t(lang, "form.required"),
                Style::default().fg(Color::DarkGray),
            ))),
            area,
            &mut ParagraphState::new(),
        );
    }
}

// ── Hint bar ──────────────────────────────────────────────────────────────────

fn render_hint(f: &mut RenderCtx<'_>, state: &AppState, area: Rect) {
    let key = if state.ctrl_hint { "form.hint.ctrl" } else { "form.hint" };
    let hint_text = state.t(key);

    let line = Line::from(vec![
        Span::styled(hint_text, Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(
            "F1=Help",
            Style::default().fg(if state.help_visible { Color::Cyan } else { Color::DarkGray }),
        ),
    ]);
    f.render_stateful_widget(Paragraph::new(line).alignment(Alignment::Center), area, &mut ParagraphState::new());
}
