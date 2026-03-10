// Task Wizard screen — progressive resource setup.
//
// Layout:
//   ┌─────────────────────────────────────────────────────────────────┐
//   │  FSN – Wizard                                           [DE]   │
//   ├─────────────────────────────────────────────────────────────────┤
//   │  [✅ Projekt]  [▶ Host]  [⏳ Proxy]  [⏳ IAM]                   │
//   ├─────────────────────────────────────────────────────────────────┤
//   │  [form.tab.service] [form.tab.network] [form.tab.env]           │
//   │  ─────────────────────────────────────────────────────          │
//   │  (active task's form fields)                                    │
//   │                                                   [Speichern]   │
//   ├─────────────────────────────────────────────────────────────────┤
//   │  hint bar                                                       │
//   └─────────────────────────────────────────────────────────────────┘

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use crate::ui::render_ctx::RenderCtx;

use crate::app::{AppState, Lang};
use crate::task_queue::TaskState;
use crate::ui::widgets;

pub fn render(f: &mut RenderCtx<'_>, state: &mut AppState, area: Rect) {
    let Some(ref _queue) = state.task_queue else { return };

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(3), // task tab bar
            Constraint::Length(3), // form tab bar
            Constraint::Min(1),    // form fields
            Constraint::Length(1), // error line
            Constraint::Length(1), // hint bar
        ])
        .split(area);

    let lang = state.lang;
    render_header(f, lang, outer[0]);

    // Task tab bar (needs immutable borrow of queue)
    {
        let queue = state.task_queue.as_ref().unwrap();
        render_task_bar(f, queue, lang, outer[1]);
    }

    // Take click_map from state before the mutable task_queue borrow below.
    let mut cmap = std::mem::take(&mut state.click_map);
    cmap.clear();

    // Form rendering — needs mutable access to the active task's form
    let active_idx = state.task_queue.as_ref().map(|q| q.active).unwrap_or(0);
    if let Some(queue) = state.task_queue.as_mut() {
        if let Some(task) = queue.tasks.get_mut(active_idx) {
            if let Some(ref mut form) = task.form {
                // Inner area with horizontal padding
                let padding = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(5),
                        Constraint::Percentage(90),
                        Constraint::Percentage(5),
                    ])
                    .split(outer[3]);

                super::new_project::render_tabs(f, lang, form, outer[2]);
                super::new_project::render_fields(f, form, padding[1], lang, &mut cmap);
                super::new_project::render_error(f, lang, form, outer[4]);
            }
        }
    }

    state.click_map = cmap;

    render_hint(f, state.ctrl_hint, state.help_visible, lang, outer[5]);
}

// ── Header ────────────────────────────────────────────────────────────────────

fn render_header(f: &mut RenderCtx<'_>, lang: Lang, area: Rect) {
    let title = Line::from(vec![
        Span::styled(" FreeSynergy.Node ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("– ", Style::default().fg(Color::DarkGray)),
        Span::styled(crate::i18n::t(lang, "wizard.title"),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]);
    f.render_stateful_widget(
        Paragraph::new(title)
            .block(Block::default().borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray))),
        area,
        &mut ParagraphState::new(),
    );

    let lang_area = Rect { x: area.right().saturating_sub(6), y: area.y + 1, width: 4, height: 1 };
    f.render_stateful_widget(
        Paragraph::new(Line::from(widgets::lang_button_raw(lang))),
        lang_area,
        &mut ParagraphState::new(),
    );
}

// ── Task tab bar ──────────────────────────────────────────────────────────────

fn render_task_bar(f: &mut RenderCtx<'_>, queue: &crate::task_queue::TaskQueue, lang: Lang, area: Rect) {
    let inner = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray))
        .inner(area);

    f.render_widget(
        Block::default().borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
        area,
    );

    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    for (i, task) in queue.tasks.iter().enumerate() {
        let is_active = i == queue.active;
        let label    = task.label(lang);

        let (icon, style) = match (task.state, is_active) {
            (TaskState::Done, _) =>
                ("✅ ", Style::default().fg(Color::Green)),
            (TaskState::Active, _) =>
                ("▶ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            (TaskState::Pending, _) =>
                ("⏳ ", Style::default().fg(Color::DarkGray)),
        };

        let bg = if is_active {
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            style
        };

        spans.push(Span::styled(format!(" {}{} ", icon, label), bg));
        spans.push(Span::styled("  ", Style::default())); // gap between tabs
    }

    f.render_stateful_widget(Paragraph::new(Line::from(spans)), inner, &mut ParagraphState::new());
}

// ── Hint bar ──────────────────────────────────────────────────────────────────

fn render_hint(f: &mut RenderCtx<'_>, ctrl_hint: bool, help_visible: bool, lang: Lang, area: Rect) {
    let key = if ctrl_hint { "form.hint.ctrl" } else { "wizard.hint" };
    let line = Line::from(vec![
        Span::styled(crate::i18n::t(lang, key), Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(
            "F1=Hilfe",
            Style::default().fg(if help_visible { Color::Cyan } else { Color::DarkGray }),
        ),
    ]);
    f.render_stateful_widget(Paragraph::new(line).alignment(Alignment::Center), area, &mut ParagraphState::new());
}
