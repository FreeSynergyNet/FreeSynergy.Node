// Select input node — single-choice field with popup dialog.
//
// Design Pattern: Bridge — delegates all selection UI to SelectionPopup (Strategy).
// Rendering the popup (radio-style) is isolated in selection_popup.rs.
// This node only owns the field identity/value and wires FormNode to the popup.
//
// UX: focused field shows current value + "▼" hint.
//     ↓/↑/Enter opens a centered popup with radio-style items.
//     Inside popup: ↑↓=navigate, Enter/→=confirm, Esc/←=cancel.

use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use crate::app::Lang;
use crate::ui::form_node::{handle_form_nav, FormAction, FormNode};
use crate::ui::nodes::selection_popup::{SelectionPopup, SelectionResult};
use crate::ui::render_ctx::RenderCtx;

#[derive(Debug)]
pub struct SelectInputNode {
    pub key:        &'static str,
    pub label_key:  &'static str,
    pub hint_key:   Option<&'static str>,
    pub tab:        usize,
    pub required:   bool,
    pub value:      String,
    /// Available choices. `Vec<String>` supports static and runtime-computed options.
    pub options:    Vec<String>,
    /// Maps an option code to a human-readable label for display.
    pub display_fn: Option<fn(&str) -> &'static str>,
    pub col_span:   u8,
    pub min_width:  u16,
    /// Popup state (Strategy).
    popup: SelectionPopup,
}

impl SelectInputNode {
    pub fn new(
        key:       &'static str,
        label_key: &'static str,
        tab:       usize,
        required:  bool,
        options:   Vec<String>,
    ) -> Self {
        let value = options.first().cloned().unwrap_or_default();
        Self {
            key, label_key, hint_key: None, tab, required,
            value, options, display_fn: None,
            col_span: 12, min_width: 0,
            popup: SelectionPopup::single(),
        }
    }

    // ── Builder helpers ────────────────────────────────────────────────────

    pub fn hint(mut self, k: &'static str) -> Self { self.hint_key = Some(k); self }

    pub fn default_val(mut self, v: &str) -> Self {
        self.value = v.to_string();
        self
    }

    pub fn display(mut self, f: fn(&str) -> &'static str) -> Self {
        self.display_fn = Some(f);
        self
    }

    pub fn col(mut self, n: u8) -> Self { self.col_span = n.min(12).max(1); self }

    pub fn min_w(mut self, n: u16) -> Self { self.min_width = n; self }

    // ── Internal ───────────────────────────────────────────────────────────

    fn current_idx(&self) -> usize {
        self.options.iter().position(|o| o == &self.value).unwrap_or(0)
    }

    fn human_label(&self) -> &str {
        if let Some(f) = self.display_fn {
            let s = f(&self.value);
            if !s.is_empty() { return s; }
        }
        &self.value
    }
}

impl FormNode for SelectInputNode {
    fn key(&self)       -> &'static str         { self.key }
    fn label_key(&self) -> &'static str         { self.label_key }
    fn col_span(&self)  -> u8                   { self.col_span }
    fn min_width(&self) -> u16                  { self.min_width }
    fn hint_key(&self)  -> Option<&'static str> { self.hint_key }
    fn tab(&self)       -> usize                { self.tab }
    fn required(&self)  -> bool                 { self.required }

    fn value(&self)           -> &str { &self.value }
    fn effective_value(&self) -> &str { &self.value }

    fn set_value(&mut self, v: &str) { self.value = v.to_string(); }
    fn is_dirty(&self)  -> bool      { false }
    fn set_dirty(&mut self, _v: bool) {}

    fn preferred_height(&self) -> u16 { 4 }

    fn render(&mut self, f: &mut RenderCtx<'_>, area: Rect, focused: bool, lang: Lang) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(1)])
            .split(area);

        let label_text  = crate::i18n::t(lang, self.label_key);
        let req_suffix  = if self.required { " *" } else { "" };
        let label_style = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let title = Line::from(Span::styled(
            format!(" {}{} ", label_text, req_suffix),
            label_style,
        ));
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let display = self.human_label();
        let input_line = if focused {
            Line::from(vec![
                Span::styled(display.to_string(), Style::default().fg(Color::White)),
                Span::styled(" ▼", Style::default().fg(Color::Cyan)),
            ])
        } else {
            Line::from(Span::styled(display.to_string(), Style::default().fg(Color::White)))
        };
        f.render_stateful_widget(
            Paragraph::new(input_line)
                .block(Block::default().borders(Borders::ALL).border_style(border_style).title(title)),
            rows[0],
            &mut ParagraphState::new(),
        );

        // Hint line below (always shown — popup appears above content, not in this space)
        if let Some(hk) = self.hint_key {
            f.render_stateful_widget(
                Paragraph::new(Line::from(Span::styled(
                    crate::i18n::t(lang, hk),
                    Style::default().fg(Color::DarkGray),
                ))),
                rows[1],
                &mut ParagraphState::new(),
            );
        }
    }

    /// Render the popup — centered on the full terminal. Called after all fields are rendered.
    fn render_overlay(&mut self, f: &mut RenderCtx<'_>, _available: Rect, lang: Lang) {
        self.popup.render(f, &self.options, self.display_fn, self.label_key, lang);
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        // Popup swallows all keys while open — global nav (Ctrl+S etc.) is bypassed too.
        if self.popup.is_open {
            return match self.popup.handle_key(key, &self.options) {
                SelectionResult::Accepted(v) => { self.value = v; FormAction::AcceptAndNext }
                SelectionResult::Rejected    => FormAction::Consumed,
                SelectionResult::Consumed    => FormAction::Consumed,
                _                            => FormAction::Consumed,
            };
        }

        // When closed: check global nav first.
        if let Some(nav) = handle_form_nav(key) { return nav; }

        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Enter => {
                let idx = self.current_idx();
                self.popup.open(idx, HashSet::new());
                FormAction::Consumed
            }
            KeyCode::Tab     => FormAction::FocusNext,
            KeyCode::BackTab => FormAction::FocusPrev,
            KeyCode::Esc     => FormAction::Cancel,
            KeyCode::Char('l') | KeyCode::Char('L') => FormAction::LangToggle,
            _ => FormAction::Unhandled,
        }
    }
}
