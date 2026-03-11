// Multi-select input node — checkbox popup for multiple choices.
//
// Design Pattern: Bridge — delegates all selection UI to SelectionPopup (Strategy).
// Rendering the popup (checkbox-style) is isolated in selection_popup.rs.
//
// Value is stored as a comma-separated list of selected option codes.
// Example: options = ["nginx", "caddy", "haproxy"], value = "nginx,haproxy"
//
// UX: closed field shows selected count or comma list.
//     ↓/↑/Enter opens centered popup with checkbox items.
//     Space=toggle, Enter/→=confirm, Esc/←=cancel.

use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use crate::app::Lang;
use crate::ui::form_node::{handle_selection_nav, FormAction, FormNode};
use crate::ui::nodes::selection_popup::{SelectionPopup, SelectionResult};
use crate::ui::render_ctx::RenderCtx;
use crate::ui::widgets::{node_block, render_hint_opt};

#[derive(Debug)]
pub struct MultiSelectInputNode {
    pub key:        &'static str,
    pub label_key:  &'static str,
    pub hint_key:   Option<&'static str>,
    pub tab:        usize,
    pub required:   bool,
    /// Comma-separated selected option codes, e.g. "nginx,haproxy".
    pub value:      String,
    /// All available choices.
    pub options:    Vec<String>,
    /// Maps an option code to a human-readable label.
    pub display_fn: Option<fn(&str) -> &'static str>,
    pub col_span:   u8,
    pub min_width:  u16,
    /// Popup state (Strategy).
    popup: SelectionPopup,
}

impl MultiSelectInputNode {
    pub fn new(
        key:       &'static str,
        label_key: &'static str,
        tab:       usize,
        required:  bool,
        options:   Vec<String>,
    ) -> Self {
        Self {
            key, label_key, hint_key: None, tab, required,
            value: String::new(), options, display_fn: None,
            col_span: 12, min_width: 0,
            popup: SelectionPopup::multi(),
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

    /// Parse `self.value` into a set of indices into `self.options`.
    fn checked_indices(&self) -> HashSet<usize> {
        self.value.split(',')
            .filter(|s| !s.is_empty())
            .filter_map(|code| self.options.iter().position(|o| o == code))
            .collect()
    }

    /// Display string: shows human labels of selected items, or placeholder.
    fn display_value(&self, lang: Lang) -> String {
        let selected: Vec<&str> = self.value.split(',')
            .filter(|s| !s.is_empty())
            .map(|code| {
                if let Some(f) = self.display_fn { f(code) } else { code }
            })
            .collect();
        if selected.is_empty() {
            crate::i18n::t(lang, "form.multiselect.none").to_string()
        } else {
            selected.join(", ")
        }
    }
}

impl FormNode for MultiSelectInputNode {
    fn key(&self)       -> &'static str         { self.key }
    fn label_key(&self) -> &'static str         { self.label_key }
    fn hint_key(&self)  -> Option<&'static str> { self.hint_key }
    fn tab(&self)       -> usize                { self.tab }
    fn required(&self)  -> bool                 { self.required }
    fn col_span(&self)  -> u8                   { self.col_span }
    fn min_width(&self) -> u16                  { self.min_width }

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

        let block = node_block(self.label_key, self.required, focused, lang);

        let display = self.display_value(lang);
        let input_line = if focused {
            Line::from(vec![
                Span::styled(display, Style::default().fg(Color::White)),
                Span::styled(" ▼", Style::default().fg(Color::Cyan)),
            ])
        } else {
            Line::from(Span::styled(display, Style::default().fg(Color::White)))
        };
        f.render_stateful_widget(
            Paragraph::new(input_line).block(block),
            rows[0],
            &mut ParagraphState::new(),
        );

        render_hint_opt(f, rows[1], self.hint_key, lang);
    }

    fn render_overlay(&mut self, f: &mut RenderCtx<'_>, _available: Rect, lang: Lang) {
        self.popup.render(f, &self.options, self.display_fn, self.label_key, lang);
    }

    fn has_open_popup(&self) -> bool { self.popup.is_open }

    fn handle_popup_mouse(&mut self, event: crossterm::event::MouseEvent) -> Option<FormAction> {
        use crate::ui::nodes::selection_popup::SelectionResult;
        match self.popup.handle_mouse(event, &self.options)? {
            SelectionResult::AcceptedMulti(values) => {
                self.value = values.join(",");
                // AcceptAndNext — same as keyboard path: on_change fires and focus advances.
                Some(FormAction::AcceptAndNext)
            }
            SelectionResult::Rejected => Some(FormAction::Consumed),
            SelectionResult::Consumed => Some(FormAction::Consumed),
            _                         => Some(FormAction::Consumed),
        }
    }

    fn handle_mouse(&mut self, event: crossterm::event::MouseEvent, _area: Rect) -> FormAction {
        use crossterm::event::{MouseButton, MouseEventKind};
        if event.kind == MouseEventKind::Down(MouseButton::Left) {
            let checked = self.checked_indices();
            self.popup.open(0, checked);
            return FormAction::Consumed;
        }
        FormAction::Unhandled
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        if self.popup.is_open {
            return match self.popup.handle_key(key, &self.options) {
                SelectionResult::AcceptedMulti(values) => {
                    self.value = values.join(",");
                    FormAction::AcceptAndNext
                }
                SelectionResult::Rejected | SelectionResult::Consumed => FormAction::Consumed,
                _ => FormAction::Consumed,
            };
        }

        // Shared nav for selection nodes: Ctrl+S/←/→, Tab, BackTab, Esc, L/l.
        if let Some(nav) = handle_selection_nav(key) { return nav; }

        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Enter => {
                let checked = self.checked_indices();
                self.popup.open(0, checked);
                FormAction::Consumed
            }
            _ => FormAction::Unhandled,
        }
    }
}
