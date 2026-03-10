// Text input node — single-line editable field with optional secret masking.
//
// Handles cursor movement, insert/delete, and returns ValueChanged so the
// parent ResourceForm can call the on_change hook for smart defaults.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::ui::render_ctx::RenderCtx;

use crate::app::Lang;
use crate::ui::form_node::{handle_form_nav, FormAction, FormNode};

#[derive(Debug)]
pub struct TextInputNode {
    pub key:       &'static str,
    pub label_key: &'static str,
    pub hint_key:  Option<&'static str>,
    pub tab:       usize,
    pub required:  bool,
    pub value:     String,
    pub default:   String,
    pub cursor:    usize,
    pub dirty:     bool,
    /// Display value as bullet characters (passwords).
    pub secret:    bool,
    /// Maximum allowed character count (0 = unlimited).
    pub max_len:   usize,
}

impl TextInputNode {
    pub fn new(
        key:       &'static str,
        label_key: &'static str,
        tab:       usize,
        required:  bool,
    ) -> Self {
        Self {
            key, label_key, hint_key: None, tab, required,
            value: String::new(), default: String::new(),
            cursor: 0, dirty: false, secret: false, max_len: 0,
        }
    }

    // ── Builder helpers ────────────────────────────────────────────────────

    pub fn hint(mut self, k: &'static str) -> Self { self.hint_key = Some(k); self }

    pub fn default_val(mut self, v: &str) -> Self {
        self.value   = v.to_string();
        self.default = v.to_string();
        self.cursor  = v.len();
        self
    }

    pub fn pre_filled(mut self, v: &str) -> Self {
        self.value   = v.to_string();
        self.default = v.to_string();
        self.cursor  = v.len();
        self.dirty   = true;
        self
    }

    pub fn secret(mut self) -> Self { self.secret = true; self }

    /// Set maximum allowed character count (0 = unlimited).
    pub fn max_len(mut self, n: usize) -> Self { self.max_len = n; self }

    // ── Internal helpers ───────────────────────────────────────────────────

    fn display_value(&self) -> String {
        if self.secret { "•".repeat(self.value.len()) } else { self.value.clone() }
    }

    fn insert_char(&mut self, c: char) {
        if self.max_len > 0 && self.value.chars().count() >= self.max_len { return; }
        self.value.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.dirty = true;
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.value[..self.cursor]
                .char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            self.value.remove(prev);
            self.cursor = prev;
            self.dirty = true;
        }
    }

    fn delete_char(&mut self) {
        if self.cursor < self.value.len() {
            let next = self.value[self.cursor..].chars().next()
                .map(|c| self.cursor + c.len_utf8()).unwrap_or(self.cursor);
            self.value.drain(self.cursor..next);
            self.dirty = true;
        }
    }

    fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.value[..self.cursor]
                .char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
        }
    }

    fn cursor_right(&mut self) {
        if self.cursor < self.value.len() {
            let next = self.value[self.cursor..].chars().next()
                .map(|c| self.cursor + c.len_utf8()).unwrap_or(self.cursor);
            self.cursor = next;
        }
    }
}

impl FormNode for TextInputNode {
    fn key(&self)       -> &'static str         { self.key }
    fn label_key(&self) -> &'static str         { self.label_key }
    fn hint_key(&self)  -> Option<&'static str> { self.hint_key }
    fn tab(&self)       -> usize                { self.tab }
    fn required(&self)  -> bool                 { self.required }

    fn value(&self) -> &str { &self.value }

    fn effective_value(&self) -> &str {
        if self.value.trim().is_empty() && !self.default.is_empty() {
            &self.default
        } else {
            &self.value
        }
    }

    fn set_value(&mut self, v: &str) {
        self.value  = v.to_string();
        self.cursor = v.len();
    }

    fn is_dirty(&self) -> bool     { self.dirty }
    fn set_dirty(&mut self, v: bool) { self.dirty = v; }

    fn preferred_height(&self) -> u16 { 4 } // box-with-title(3) + hint(1)

    fn render(&mut self, f: &mut RenderCtx<'_>, area: Rect, focused: bool, lang: Lang) {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // input box (label in title)
                Constraint::Length(1), // hint
            ])
            .split(area);

        // Label displayed as block title
        let label_text = crate::i18n::t(lang, self.label_key);
        let req_suffix  = if self.required { " *" } else { "" };
        let label_style = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let title = Line::from(vec![
            Span::styled(format!(" {}{} ", label_text, req_suffix), label_style),
        ]);

        // Input box with cursor
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let display = self.display_value();
        let input_line = if focused {
            let before = &display[..self.cursor.min(display.len())];
            let after  = &display[self.cursor.min(display.len())..];
            Line::from(vec![
                Span::styled(before.to_string(), Style::default().fg(Color::White)),
                Span::styled("█",               Style::default().fg(Color::Cyan)),
                Span::styled(after.to_string(),  Style::default().fg(Color::White)),
            ])
        } else if display.is_empty() {
            Line::from(Span::raw(""))
        } else {
            Line::from(Span::styled(display, Style::default().fg(Color::White)))
        };
        f.render_widget(
            Paragraph::new(input_line)
                .block(Block::default().borders(Borders::ALL).border_style(border_style).title(title)),
            rows[0],
        );

        // Hint
        if let Some(hk) = self.hint_key {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    crate::i18n::t(lang, hk),
                    Style::default().fg(Color::DarkGray),
                ))),
                rows[1],
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        // Ctrl+S=Submit, Ctrl+←/→=TabPrev/Next — consistent across all nodes.
        if let Some(nav) = handle_form_nav(key) { return nav; }

        use KeyModifiers as KM;
        match key.code {
            // Tab switches between form tabs; Enter moves to the next field.
            KeyCode::Tab     => FormAction::TabNext,
            KeyCode::BackTab => FormAction::TabPrev,
            KeyCode::Esc     => FormAction::Cancel,
            KeyCode::Enter   => FormAction::FocusNext,

            // Cursor movement (no value change)
            KeyCode::Left  => { self.cursor_left();  FormAction::Consumed }
            KeyCode::Right => { self.cursor_right(); FormAction::Consumed }
            KeyCode::Home  => { self.cursor = 0;                    FormAction::Consumed }
            KeyCode::End   => { self.cursor = self.value.len();      FormAction::Consumed }

            // Editing (triggers on_change via ValueChanged)
            KeyCode::Backspace => { self.backspace();   FormAction::ValueChanged }
            KeyCode::Delete    => { self.delete_char(); FormAction::ValueChanged }

            KeyCode::Char(c) if !key.modifiers.contains(KM::CONTROL) => {
                self.insert_char(c);
                FormAction::ValueChanged
            }

            _ => FormAction::Unhandled,
        }
    }
}
