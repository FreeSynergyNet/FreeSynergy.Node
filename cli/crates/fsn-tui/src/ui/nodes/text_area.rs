// TextAreaNode — multi-line text input.
//
// Analogous to HTML <textarea rows="N">.
// Use Tab to advance focus; Enter inserts a newline; Ctrl+Enter submits.
//
// preferred_height = visible_lines + 3  (2 borders + 1 hint row)
//
// Internal design: `lines: Vec<String>` is the canonical content.
// `cache: String` = lines.join("\n") — kept in sync after every edit so
// the `FormNode::value()` / `effective_value()` contract (&str) is satisfied.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::Lang;
use crate::ui::form_node::{FormAction, FormNode};

const DEFAULT_ROWS: u16 = 4;

#[derive(Debug)]
pub struct TextAreaNode {
    pub key:           &'static str,
    pub label_key:     &'static str,
    pub hint_key:      Option<&'static str>,
    pub tab:           usize,
    pub required:      bool,
    /// Lines of content (always ≥ 1 element).
    lines:             Vec<String>,
    /// Cache: always equal to `lines.join("\n")`.
    cache:             String,
    cursor_line:       usize,
    cursor_col:        usize,
    /// How many text rows are visible in the rendered box.
    pub visible_lines: u16,
    scroll_offset:     usize,
    rect:              Option<Rect>,
    pub dirty:         bool,
}

impl TextAreaNode {
    pub fn new(
        key:       &'static str,
        label_key: &'static str,
        tab:       usize,
        required:  bool,
    ) -> Self {
        Self {
            key, label_key, hint_key: None, tab, required,
            lines: vec![String::new()],
            cache: String::new(),
            cursor_line: 0, cursor_col: 0,
            visible_lines: DEFAULT_ROWS,
            scroll_offset: 0,
            rect: None,
            dirty: false,
        }
    }

    // ── Builder helpers ────────────────────────────────────────────────────

    pub fn hint(mut self, k: &'static str)  -> Self { self.hint_key = Some(k); self }
    pub fn rows(mut self, n: u16)           -> Self { self.visible_lines = n.max(1); self }

    pub fn default_val(mut self, v: &str) -> Self {
        self.load_value(v);
        self
    }

    pub fn pre_filled(mut self, v: &str) -> Self {
        self.load_value(v);
        self.dirty = true;
        self
    }

    // ── Internal helpers ───────────────────────────────────────────────────

    fn load_value(&mut self, v: &str) {
        self.lines = v.split('\n').map(|s| s.to_string()).collect();
        if self.lines.is_empty() { self.lines.push(String::new()); }
        self.cursor_line = self.lines.len().saturating_sub(1);
        self.cursor_col  = self.lines.last().map(|l| l.len()).unwrap_or(0);
        self.sync_cache();
    }

    fn sync_cache(&mut self) {
        self.cache = self.lines.join("\n");
    }

    fn clamp_cursor(&mut self) {
        self.cursor_line = self.cursor_line.min(self.lines.len().saturating_sub(1));
        let line_len = self.lines[self.cursor_line].len();
        self.cursor_col  = self.cursor_col.min(line_len);
    }

    fn ensure_scroll_visible(&mut self) {
        if self.cursor_line < self.scroll_offset {
            self.scroll_offset = self.cursor_line;
        } else if self.cursor_line >= self.scroll_offset + self.visible_lines as usize {
            self.scroll_offset = self.cursor_line + 1 - self.visible_lines as usize;
        }
    }

    fn insert_char(&mut self, c: char) {
        self.lines[self.cursor_line].insert(self.cursor_col, c);
        self.cursor_col += c.len_utf8();
        self.dirty = true;
        self.sync_cache();
    }

    fn insert_newline(&mut self) {
        let rest = self.lines[self.cursor_line][self.cursor_col..].to_string();
        self.lines[self.cursor_line].truncate(self.cursor_col);
        self.cursor_line += 1;
        self.lines.insert(self.cursor_line, rest);
        self.cursor_col = 0;
        self.dirty = true;
        self.sync_cache();
        self.ensure_scroll_visible();
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let prev = self.lines[self.cursor_line][..self.cursor_col]
                .char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            self.lines[self.cursor_line].remove(prev);
            self.cursor_col = prev;
        } else if self.cursor_line > 0 {
            let current = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            self.cursor_col   = self.lines[self.cursor_line].len();
            self.lines[self.cursor_line].push_str(&current);
            self.ensure_scroll_visible();
        } else {
            return; // nothing to delete
        }
        self.dirty = true;
        self.sync_cache();
    }

    fn delete_forward(&mut self) {
        let line_len = self.lines[self.cursor_line].len();
        if self.cursor_col < line_len {
            let next = self.lines[self.cursor_line][self.cursor_col..].chars().next()
                .map(|c| self.cursor_col + c.len_utf8()).unwrap_or(self.cursor_col);
            self.lines[self.cursor_line].drain(self.cursor_col..next);
        } else if self.cursor_line + 1 < self.lines.len() {
            let next_line = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next_line);
        } else {
            return;
        }
        self.dirty = true;
        self.sync_cache();
    }
}

impl FormNode for TextAreaNode {
    fn key(&self)       -> &'static str         { self.key }
    fn label_key(&self) -> &'static str         { self.label_key }
    fn hint_key(&self)  -> Option<&'static str> { self.hint_key }
    fn tab(&self)       -> usize                { self.tab }
    fn required(&self)  -> bool                 { self.required }

    fn value(&self)           -> &str { &self.cache }
    fn effective_value(&self) -> &str { &self.cache }

    fn set_value(&mut self, v: &str) { self.load_value(v); }
    fn is_dirty(&self)        -> bool { self.dirty }
    fn set_dirty(&mut self, v: bool)  { self.dirty = v; }

    fn set_rect(&mut self, r: Rect)     { self.rect = Some(r); }
    fn last_rect(&self) -> Option<Rect> { self.rect }

    fn is_filled(&self) -> bool {
        self.lines.iter().any(|l| !l.trim().is_empty())
    }

    fn preferred_height(&self) -> u16 {
        self.visible_lines + 3 // box(visible_lines + 2 borders) + hint(1)
    }

    fn render(&mut self, f: &mut Frame, area: Rect, focused: bool, lang: Lang) {
        self.set_rect(area);
        self.clamp_cursor();
        self.ensure_scroll_visible();

        let box_h = self.visible_lines + 2;
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(box_h), // textarea
                Constraint::Length(1),     // hint
            ])
            .split(area);

        // Label as block title
        let label_text  = crate::i18n::t(lang, self.label_key);
        let req_suffix  = if self.required { " *" } else { "" };
        let label_style = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let title = Line::from(Span::styled(format!(" {}{} ", label_text, req_suffix), label_style));

        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        // Build visible content lines
        let content: Vec<Line> = (0..self.visible_lines as usize).map(|rel| {
            let abs      = self.scroll_offset + rel;
            let line_str = self.lines.get(abs).map(|s| s.as_str()).unwrap_or("");

            if focused && abs == self.cursor_line {
                let col    = self.cursor_col.min(line_str.len());
                let before = &line_str[..col];
                let after  = &line_str[col..];
                Line::from(vec![
                    Span::styled(before.to_string(), Style::default().fg(Color::White)),
                    Span::styled("█",               Style::default().fg(Color::Cyan)),
                    Span::styled(after.to_string(),  Style::default().fg(Color::White)),
                ])
            } else {
                Line::from(Span::styled(line_str.to_string(), Style::default().fg(Color::White)))
            }
        }).collect();

        f.render_widget(
            Paragraph::new(content)
                .block(Block::default().borders(Borders::ALL).border_style(border_style).title(title)),
            rows[0],
        );

        let hint_text = if let Some(hk) = self.hint_key {
            crate::i18n::t(lang, hk)
        } else {
            crate::i18n::t(lang, "form.textarea.hint")
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hint_text,
                Style::default().fg(Color::DarkGray),
            ))),
            rows[1],
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        use KeyModifiers as KM;
        match key.code {
            // Submit: Ctrl+Enter or Alt+Enter (terminal compatibility — many terminals
            // cannot distinguish Ctrl+Enter from plain Enter).
            KeyCode::Enter if key.modifiers.intersects(KM::CONTROL | KM::ALT) => FormAction::Submit,

            KeyCode::Tab     => FormAction::FocusNext,
            KeyCode::BackTab => FormAction::FocusPrev,
            // Esc exits the textarea (back to dashboard or cancel form).
            KeyCode::Esc     => FormAction::Cancel,
            KeyCode::Left  if key.modifiers.contains(KM::CONTROL) => FormAction::TabPrev,
            KeyCode::Right if key.modifiers.contains(KM::CONTROL) => FormAction::TabNext,

            KeyCode::Up => {
                if self.cursor_line > 0 {
                    self.cursor_line -= 1;
                    self.clamp_cursor();
                    self.ensure_scroll_visible();
                }
                FormAction::Consumed
            }
            KeyCode::Down => {
                if self.cursor_line + 1 < self.lines.len() {
                    self.cursor_line += 1;
                    self.clamp_cursor();
                    self.ensure_scroll_visible();
                }
                FormAction::Consumed
            }
            KeyCode::Left => {
                if self.cursor_col > 0 {
                    let line = &self.lines[self.cursor_line];
                    self.cursor_col = line[..self.cursor_col]
                        .char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
                } else if self.cursor_line > 0 {
                    self.cursor_line -= 1;
                    self.cursor_col   = self.lines[self.cursor_line].len();
                    self.ensure_scroll_visible();
                }
                FormAction::Consumed
            }
            KeyCode::Right => {
                let line_len = self.lines[self.cursor_line].len();
                if self.cursor_col < line_len {
                    let c = self.lines[self.cursor_line][self.cursor_col..].chars().next();
                    self.cursor_col += c.map(|ch| ch.len_utf8()).unwrap_or(0);
                } else if self.cursor_line + 1 < self.lines.len() {
                    self.cursor_line += 1;
                    self.cursor_col   = 0;
                    self.ensure_scroll_visible();
                }
                FormAction::Consumed
            }
            KeyCode::Home => { self.cursor_col = 0; FormAction::Consumed }
            KeyCode::End  => {
                self.cursor_col = self.lines[self.cursor_line].len();
                FormAction::Consumed
            }

            KeyCode::Enter     => { self.insert_newline();   FormAction::ValueChanged }
            KeyCode::Backspace => { self.backspace();        FormAction::ValueChanged }
            KeyCode::Delete    => { self.delete_forward();   FormAction::ValueChanged }

            KeyCode::Char(c) if !key.modifiers.contains(KM::CONTROL) => {
                self.insert_char(c);
                FormAction::ValueChanged
            }

            _ => FormAction::Unhandled,
        }
    }
}
