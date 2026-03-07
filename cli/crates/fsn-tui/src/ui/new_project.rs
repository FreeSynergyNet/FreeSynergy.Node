// New Project form — three tabs: Projekt / Server / Optionen.
//
// ┌──────────────────────────────────────────────────────────────────┐
// │  FreeSynergy.Node – Neues Projekt                      [DE]     │
// ├──────────────────────────────────────────────────────────────────┤
// │ ┌─ Projekt ──┐  ┌─ Server ──┐  ┌─ Optionen ──┐                 │
// │                                                                  │
// │  Projektname *                                                   │
// │  ┌──────────────────────────────────────────────────────────┐   │
// │  │ myproject_                                               │   │
// │  └──────────────────────────────────────────────────────────┘   │
// │  Kurzname ohne Leerzeichen, z.B. myproject                      │
// │                                                                  │
// │  Domain *                                                        │
// │  ┌──────────────────────────────────────────────────────────┐   │
// │  │ example.com_                                             │   │
// │  └──────────────────────────────────────────────────────────┘   │
// ├──────────────────────────────────────────────────────────────────┤
// │  Tab=Nächstes Feld  ←→=Tab wechseln  Enter=Weiter  Esc=Zurück   │
// └──────────────────────────────────────────────────────────────────┘

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

use crate::app::{AppState, FormFieldType, FormTab, NewProjectForm};
use crate::ui::widgets;

pub fn render(f: &mut Frame, state: &AppState) {
    let Some(ref form) = state.new_project else { return };
    let area = f.area();

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Length(3),  // tab bar
            Constraint::Min(1),     // form fields
            Constraint::Length(1),  // error line (if any)
            Constraint::Length(1),  // hint bar
        ])
        .split(area);

    render_header(f, state, outer[0]);
    render_tabs(f, state, form, outer[1]);
    render_fields(f, state, form, outer[2]);
    render_error(f, state, form, outer[3]);
    render_hint(f, state, outer[4]);
}

// ── Header ────────────────────────────────────────────────────────────────────

fn render_header(f: &mut Frame, state: &AppState, area: Rect) {
    let title = Line::from(vec![
        Span::styled(" FreeSynergy.Node ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("– ",                  Style::default().fg(Color::DarkGray)),
        Span::styled(state.t("welcome.new_project"), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]);
    let header = Paragraph::new(title)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    f.render_widget(header, area);

    let lang_area = Rect { x: area.right().saturating_sub(6), y: area.y + 1, width: 4, height: 1 };
    f.render_widget(Paragraph::new(Line::from(widgets::lang_button(state))), lang_area);
}

// ── Tab bar ───────────────────────────────────────────────────────────────────

fn render_tabs(f: &mut Frame, state: &AppState, form: &NewProjectForm, area: Rect) {
    let tab_titles: Vec<Line> = (0..FormTab::count())
        .map(|i| {
            let tab = FormTab::from_index(i);
            let label = state.t(tab.i18n_key());

            // Count required-but-empty fields in this tab
            let tab_fields: Vec<_> = form.fields.iter()
                .filter(|f| f.tab == tab)
                .collect();
            let has_missing = tab_fields.iter().any(|f| f.required && f.value.trim().is_empty());
            let is_active = i == form.active_tab;

            let text = if has_missing && !is_active {
                format!(" {} ⚠ ", label)
            } else {
                format!(" {} ", label)
            };

            if is_active {
                Line::from(Span::styled(text, Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)))
            } else if has_missing {
                Line::from(Span::styled(text, Style::default().fg(Color::Yellow)))
            } else {
                Line::from(Span::styled(text, Style::default().fg(Color::DarkGray)))
            }
        })
        .collect();

    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)))
        .select(form.active_tab)
        .divider(Span::styled("  ", Style::default()));
    f.render_widget(tabs, area);
}

// ── Form fields ───────────────────────────────────────────────────────────────

fn render_fields(f: &mut Frame, state: &AppState, form: &NewProjectForm, area: Rect) {
    let padding = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(5),
            Constraint::Percentage(90),
            Constraint::Percentage(5),
        ])
        .split(area);

    let inner = padding[1];
    let tab_indices = form.tab_field_indices();

    // Build constraints: each field = label(1) + input(3) + hint(1) + gap(1) = 5 lines
    let per_field = 5usize;
    let total_height = tab_indices.len() * per_field;
    let mut field_areas: Vec<Rect> = Vec::new();

    let mut y = inner.y;
    for _ in 0..tab_indices.len() {
        if y + per_field as u16 > inner.bottom() { break; }
        field_areas.push(Rect { x: inner.x, y, width: inner.width, height: per_field as u16 });
        y += per_field as u16;
    }

    // Also compute submit button area after last field
    let _ = total_height; // suppress warning

    for (slot, &field_idx) in tab_indices.iter().enumerate() {
        let Some(area) = field_areas.get(slot) else { break };
        let field = &form.fields[field_idx];
        let focused = form.active_field == slot;

        // Layout per field: label / input box / hint
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // label
                Constraint::Length(3),  // input box (with border)
                Constraint::Length(1),  // hint text
            ])
            .split(*area);

        // Label line: "Feldname *" (required) or "Feldname (optional)"
        let req_marker = if field.required {
            Span::styled(" *", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        } else {
            Span::styled("", Style::default())
        };
        let label_style = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let label_line = Line::from(vec![
            Span::styled(state.t(field.label_key), label_style),
            req_marker,
        ]);
        f.render_widget(Paragraph::new(label_line), rows[0]);

        // Input box
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let input_block = Block::default().borders(Borders::ALL).border_style(border_style);

        let display_value = match field.field_type {
            FormFieldType::Secret => "•".repeat(field.value.len()),
            FormFieldType::Select => field.value.clone(),
            _ => field.value.clone(),
        };

        // Show cursor as a trailing block █ when focused
        let input_text = if focused {
            let before_cursor = &display_value[..field.cursor.min(display_value.len())];
            let after_cursor  = &display_value[field.cursor.min(display_value.len())..];
            Line::from(vec![
                Span::styled(before_cursor.to_string(), Style::default().fg(Color::White)),
                Span::styled("█", Style::default().fg(Color::Cyan)),
                Span::styled(after_cursor.to_string(),  Style::default().fg(Color::White)),
            ])
        } else if display_value.is_empty() {
            Line::from(Span::styled("", Style::default().fg(Color::DarkGray)))
        } else {
            Line::from(Span::styled(display_value, Style::default().fg(Color::White)))
        };

        let input = Paragraph::new(input_text).block(input_block);
        f.render_widget(input, rows[1]);

        // Hint line
        if let Some(hint_key) = field.hint_key {
            let hint = Paragraph::new(Line::from(Span::styled(
                state.t(hint_key),
                Style::default().fg(Color::DarkGray),
            )));
            f.render_widget(hint, rows[2]);
        }
    }

    // Submit button at the bottom (only on last tab — Options)
    if form.active_tab == FormTab::count() - 1 {
        if let Some(last) = field_areas.last() {
            let btn_y = last.y + last.height + 1;
            if btn_y + 3 <= inner.bottom() {
                let btn_area = Rect { x: inner.x, y: btn_y, width: inner.width / 3, height: 3 };
                let missing = form.missing_required();
                let disabled = !missing.is_empty();
                let btn = Paragraph::new(widgets::button_line(state.t("form.submit"), true, disabled))
                    .block(Block::default().borders(Borders::ALL).border_style(
                        if disabled { Style::default().fg(Color::DarkGray) } else { Style::default().fg(Color::Green) }
                    ))
                    .alignment(Alignment::Center);
                f.render_widget(btn, btn_area);
            }
        }
    }
}

// ── Error line ────────────────────────────────────────────────────────────────

fn render_error(f: &mut Frame, state: &AppState, form: &NewProjectForm, area: Rect) {
    if let Some(ref err) = form.error {
        let line = Line::from(vec![
            Span::styled(format!("  {} ", state.t("form.error")), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::styled(err.as_str(), Style::default().fg(Color::Red)),
        ]);
        f.render_widget(Paragraph::new(line), area);
    } else {
        let req = Paragraph::new(Line::from(Span::styled(
            state.t("form.required"),
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(req, area);
    }
}

// ── Hint bar ──────────────────────────────────────────────────────────────────

fn render_hint(f: &mut Frame, state: &AppState, area: Rect) {
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(state.t("form.hint"), Style::default().fg(Color::DarkGray))))
            .alignment(Alignment::Center),
        area,
    );
}
