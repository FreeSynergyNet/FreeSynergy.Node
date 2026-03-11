// UI rendering — dispatches to screen-specific renderers.
//
// Render takes `&mut AppState` because FormNode::render(&mut self, ...) needs
// to store the last rendered Rect for mouse hit-testing (layout cache).
//
// Layout with help sidebar:
//   ┌─────────────────────────┬──────────────────────────────┐
//   │  main content           │  F1 Help sidebar (30 cols)   │
//   └─────────────────────────┴──────────────────────────────┘
// When help_visible=false the sidebar column is omitted.

pub mod anim;
pub mod components;
pub mod cursor;
pub mod dashboard;
pub mod detail;
pub mod form_node;
pub mod help_sidebar;
pub mod layout;
pub mod logs;
pub mod new_project;
pub mod nodes;
pub mod render_ctx;
pub mod settings_screen;
pub mod style;
pub mod welcome;
pub mod widgets;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use crate::app::{AppState, OverlayLayer, Screen};
use render_ctx::RenderCtx;
use components::{Component, NotifStack};

// ── OverlayLayer rendering — each variant renders itself ──────────────────────

impl OverlayLayer {
    /// Render this overlay layer on top of the main content.
    /// Analogous to `Element::render()` — the caller just iterates the stack.
    fn render(&self, f: &mut RenderCtx<'_>, state: &AppState) {
        match self {
            OverlayLayer::Logs(_)            => logs::render(f, state),
            OverlayLayer::Confirm { .. }     => render_confirm(f, state),
            OverlayLayer::Deploy(_)          => render_deploy(f, state),
            OverlayLayer::NewResource { .. } => render_new_resource(f, state),
            OverlayLayer::ContextMenu { .. } => render_context_menu(f, state),
        }
    }
}

pub fn render(f: &mut RenderCtx<'_>, state: &mut AppState) {
    let full = f.area();

    // Dashboard handles F1 help panel internally (body-area only).
    // All other screens use a full-screen horizontal split when help is visible.
    match state.screen {
        Screen::Dashboard  => dashboard::render(f, state, full),
        Screen::Welcome    => render_with_help(f, state, full, |f, s, a| welcome::render(f, s, a)),
        Screen::NewProject => render_with_help(f, state, full, |f, s, a| new_project::render(f, s, a)),
        Screen::Settings   => render_with_help(f, state, full, |f, s, a| settings_screen::render(f, s, a)),
    }

    // Overlay layers drawn on top (Ebene system).
    // Collect indices first to avoid borrow conflict (render borrows state immutably).
    let layer_count = state.overlay_stack.len();
    for i in 0..layer_count {
        // SAFETY: index is valid, no mutation during iteration
        let layer = &state.overlay_stack[i];
        layer.render(f, state);
    }

    // Toast notifications — always on top, top-right corner.
    NotifStack.render(f, full, state);
}

/// Renders a non-dashboard screen with the optional full-screen help sidebar.
fn render_with_help<R>(f: &mut RenderCtx<'_>, state: &mut AppState, area: Rect, render_fn: R)
where
    R: FnOnce(&mut RenderCtx<'_>, &mut AppState, Rect),
{
    let (main_area, help_area) = if state.help_visible && area.width > help_sidebar::SIDEBAR_WIDTH + 20 {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Min(20),
                Constraint::Length(help_sidebar::SIDEBAR_WIDTH),
            ])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    render_fn(f, state, main_area);

    if let Some(help_area) = help_area {
        let kind    = state.active_form().map(|f| f.kind);
        let foc_key = state.active_form()
            .and_then(|f| f.focused_node())
            .map(|n| n.key());
        let sections = help_sidebar::build_help(state.screen, kind, foc_key, state.lang);
        help_sidebar::render_help_sidebar(f, help_area, &sections, state.lang);
    }
}

fn render_new_resource(f: &mut RenderCtx<'_>, state: &AppState) {
    use ratatui::{
        layout::{Alignment, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, BorderType, Borders, Clear},
    };
    use rat_widget::paragraph::{Paragraph, ParagraphState};
    use crate::app::{NEW_RESOURCE_ITEMS, OverlayLayer};

    let selected = match state.top_overlay() {
        Some(OverlayLayer::NewResource { selected }) => *selected,
        _ => return,
    };

    let area    = f.area();
    let width   = 36u16;
    // height: title-border(1) + gap(1) + items + gap(1) + hint(1) + border(1) = items + 5
    let height  = NEW_RESOURCE_ITEMS.len() as u16 + 5;
    let popup   = Rect {
        x:      area.width.saturating_sub(width) / 2,
        y:      area.height.saturating_sub(height) / 2,
        width,
        height,
    };

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            format!(" {} ", state.t("new.resource.title")),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    // Option rows
    let mut lines: Vec<Line> = vec![Line::from("")];
    for (i, &(key, _)) in NEW_RESOURCE_ITEMS.iter().enumerate() {
        let is_sel    = i == selected;
        let marker    = if is_sel { "▶ " } else { "  " };
        let label     = state.t(key);
        let row_style = if is_sel {
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let text   = format!("{}{}", marker, label);
        let padded = format!("{:<w$}", text, w = (inner.width as usize).saturating_sub(0));
        lines.push(Line::from(Span::styled(padded, row_style)));
    }

    // Hint
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        state.t("new.resource.hint"),
        Style::default().fg(Color::DarkGray),
    )));

    f.render_stateful_widget(
        Paragraph::new(lines).alignment(Alignment::Left),
        inner,
        &mut ParagraphState::new(),
    );
}

fn render_confirm(f: &mut RenderCtx<'_>, state: &AppState) {
    use ratatui::{
        layout::{Alignment, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Clear},
    };
    use rat_widget::paragraph::{Paragraph, ParagraphState};

    let Some((msg_key, data, _)) = state.confirm_overlay() else { return };
    let display_msg = if let Some(d) = data {
        format!("{} '{}'", state.t(msg_key), d)
    } else {
        state.t(msg_key).to_string()
    };
    let area = f.area();
    let popup = Rect {
        x:      area.width / 4,
        y:      area.height / 2 - 2,
        width:  area.width / 2,
        height: 3,
    };

    f.render_widget(Clear, popup);
    f.render_stateful_widget(
        Paragraph::new(Line::from(Span::styled(
            display_msg,
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )))
        .block(Block::default().borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)))
        .alignment(Alignment::Center),
        popup,
        &mut ParagraphState::new(),
    );
}

fn render_deploy(f: &mut RenderCtx<'_>, state: &AppState) {
    use ratatui::{
        layout::{Alignment, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Clear},
    };
    use rat_widget::paragraph::{Paragraph, ParagraphState};

    let ds = state.overlay_stack.iter().rev().find_map(|o| {
        if let OverlayLayer::Deploy(ref d) = o { Some(d) } else { None }
    });
    let Some(ds) = ds else { return };

    let area      = f.area();
    let width     = (area.width * 2 / 3).max(50).min(area.width.saturating_sub(4));
    let log_lines = ds.log.len() as u16;
    let height    = (log_lines + 4).max(6).min(area.height.saturating_sub(4));
    let popup = Rect {
        x:      area.width.saturating_sub(width) / 2,
        y:      area.height.saturating_sub(height) / 2,
        width,
        height,
    };

    let border_color = if ds.done {
        if ds.success { Color::Green } else { Color::Red }
    } else {
        Color::Cyan
    };

    let status_icon = if ds.done {
        if ds.success { "✓" } else { "✗" }
    } else {
        state.anim.spinner()
    };
    let title = format!(" {} {} — {} ", state.t("deploy.title"), status_icon, ds.target);

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(&title, Style::default().fg(border_color).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner_area = block.inner(popup);
    f.render_widget(block, popup);

    // Log lines
    let log_area = Rect {
        x:      inner_area.x,
        y:      inner_area.y,
        width:  inner_area.width,
        height: inner_area.height.saturating_sub(1),
    };
    let lines: Vec<Line> = ds.log.iter().map(|l| {
        let color = if l.starts_with('✓') { Color::Green }
                    else if l.starts_with('✗') { Color::Red }
                    else { Color::White };
        Line::from(Span::styled(l.as_str(), Style::default().fg(color)))
    }).collect();
    f.render_stateful_widget(Paragraph::new(lines), log_area, &mut ParagraphState::new());

    // Hint bar at bottom
    let hint_text = if ds.done { state.t("deploy.hint") } else { state.t("deploy.running") };
    let hint_area = Rect {
        x:      inner_area.x,
        y:      inner_area.bottom().saturating_sub(1),
        width:  inner_area.width,
        height: 1,
    };
    f.render_stateful_widget(
        Paragraph::new(Line::from(Span::styled(hint_text, Style::default().fg(Color::DarkGray))))
            .alignment(Alignment::Center),
        hint_area,
        &mut ParagraphState::new(),
    );
}

// ── Context menu ──────────────────────────────────────────────────────────────
//
// Floating popup rendered at the right-click position (clamped to screen).
// To change the visual style: edit only this function.
// To change which items appear: edit mouse::context_items_for().

fn render_context_menu(f: &mut RenderCtx<'_>, state: &AppState) {
    use ratatui::{
        layout::{Alignment, Rect},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, BorderType, Borders, Clear},
    };
    use rat_widget::paragraph::{Paragraph, ParagraphState};
    use crate::app::OverlayLayer;

    let (cx, cy, items, selected) = match state.top_overlay() {
        Some(OverlayLayer::ContextMenu { x, y, items, selected, .. }) => (*x, *y, items, *selected),
        _ => return,
    };

    if items.is_empty() { return; }

    let area      = f.area();
    let max_label = items.iter()
        .map(|a| state.t(a.label_key()).chars().count())
        .max()
        .unwrap_or(8);
    let width  = (max_label as u16 + 4).min(area.width);
    let height = items.len() as u16 + 2;

    let x = cx.min(area.right().saturating_sub(width));
    let y = cy.min(area.bottom().saturating_sub(height));
    let popup = Rect { x, y, width, height };

    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let lines: Vec<Line> = items.iter().enumerate().map(|(i, action)| {
        let label  = state.t(action.label_key());
        let is_sel = i == selected;
        let style  = if is_sel {
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else if action.is_danger() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::White)
        };
        let prefix = if is_sel { "▶ " } else { "  " };
        let text   = format!("{}{}", prefix, label);
        let padded = format!("{:<w$}", text, w = inner.width as usize);
        Line::from(Span::styled(padded, style))
    }).collect();

    f.render_stateful_widget(
        Paragraph::new(lines).alignment(Alignment::Left),
        inner,
        &mut ParagraphState::new(),
    );
}
