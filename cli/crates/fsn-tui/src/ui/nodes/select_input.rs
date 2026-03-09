// Select input node — drop-down field.
//
// Dropdown lifecycle:
//   Focused but closed: field shows current value + "▼" hint, no list visible.
//   ↓ / ↑ / Enter / click on field → opens the dropdown.
//   In dropdown: ↑↓ move the pending highlight, ←/Esc close without change,
//                →/Enter/click on item accept + close.
//   Click outside dropdown while open → close without change.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::Lang;
use crate::ui::form_node::{FormAction, FormNode};

#[derive(Debug)]
pub struct SelectInputNode {
    pub key:        &'static str,
    pub label_key:  &'static str,
    pub hint_key:   Option<&'static str>,
    pub tab:        usize,
    pub required:   bool,
    pub value:      String,
    /// Available choices. `Vec<String>` supports both static and runtime-computed options.
    pub options:    Vec<String>,
    /// Maps an option code to a human-readable label.
    pub display_fn: Option<fn(&str) -> &'static str>,
    rect:           Option<Rect>,
    /// Whether the dropdown list is currently visible.
    is_open:        bool,
    /// Index of the highlighted item inside the open dropdown (not yet confirmed).
    pending_idx:    usize,
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
            value, options, display_fn: None, rect: None,
            is_open: false, pending_idx: 0,
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

    // ── Internal helpers ───────────────────────────────────────────────────

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

    fn open(&mut self) {
        self.pending_idx = self.current_idx();
        self.is_open = true;
    }

    fn close_reject(&mut self) {
        self.is_open = false;
    }

    fn close_accept(&mut self) {
        if let Some(opt) = self.options.get(self.pending_idx) {
            self.value = opt.clone();
        }
        self.is_open = false;
    }

    /// Geometry of the dropdown list (shared by render and click-test).
    fn dropdown_rect(&self, available: Rect) -> Option<Rect> {
        let input_rect      = self.rect?;
        let input_box_bottom = input_rect.y + 3;
        let avail_h = available.bottom().saturating_sub(input_box_bottom);
        let want_h  = (self.options.len() as u16 + 2).min(avail_h);
        if want_h < 3 { return None; }
        Some(Rect {
            x: input_rect.x,
            y: input_box_bottom,
            width: input_rect.width,
            height: want_h,
        })
    }
}

impl FormNode for SelectInputNode {
    fn key(&self)       -> &'static str         { self.key }
    fn label_key(&self) -> &'static str         { self.label_key }
    fn hint_key(&self)  -> Option<&'static str> { self.hint_key }
    fn tab(&self)       -> usize                { self.tab }
    fn required(&self)  -> bool                 { self.required }

    fn value(&self)           -> &str { &self.value }
    fn effective_value(&self) -> &str { &self.value }

    fn set_value(&mut self, v: &str) { self.value = v.to_string(); }
    fn is_dirty(&self)  -> bool      { false }
    fn set_dirty(&mut self, _v: bool) {}

    fn set_rect(&mut self, r: Rect)     { self.rect = Some(r); }
    fn last_rect(&self) -> Option<Rect> { self.rect }

    fn preferred_height(&self) -> u16 { 4 }

    fn render(&mut self, f: &mut Frame, area: Rect, focused: bool, lang: Lang) {
        self.set_rect(area);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(area);

        let label_text  = crate::i18n::t(lang, self.label_key);
        let req_suffix  = if self.required { " *" } else { "" };
        let label_style = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let title = Line::from(vec![
            Span::styled(format!(" {}{} ", label_text, req_suffix), label_style),
        ]);

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
        f.render_widget(
            Paragraph::new(input_line)
                .block(Block::default().borders(Borders::ALL).border_style(border_style).title(title)),
            rows[0],
        );

        // Hint only when closed (dropdown takes the space while open)
        if !self.is_open {
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
    }

    /// Render the dropdown — only when `is_open`.
    fn render_overlay(&mut self, f: &mut Frame, available: Rect, _lang: Lang) {
        if !self.is_open { return; }
        let Some(dropdown) = self.dropdown_rect(available) else { return };

        let items: Vec<ListItem> = self.options.iter().enumerate().map(|(i, opt)| {
            let label  = if let Some(f) = self.display_fn { f(opt.as_str()) } else { opt.as_str() };
            let prefix = if i == self.pending_idx { "▶ " } else { "  " };
            let style  = if i == self.pending_idx {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(format!("{}{}", prefix, label), style)))
        }).collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)));

        f.render_widget(Clear, dropdown);
        f.render_widget(list, dropdown);
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        use KeyModifiers as KM;

        if !self.is_open {
            match key.code {
                // Open dropdown
                KeyCode::Down | KeyCode::Up | KeyCode::Enter => {
                    self.open();
                    FormAction::Consumed
                }
                KeyCode::Tab     => FormAction::FocusNext,
                KeyCode::BackTab => FormAction::FocusPrev,
                KeyCode::Esc     => FormAction::Cancel,
                KeyCode::Left  if key.modifiers.contains(KM::CONTROL) => FormAction::TabPrev,
                KeyCode::Right if key.modifiers.contains(KM::CONTROL) => FormAction::TabNext,
                KeyCode::Char('l') | KeyCode::Char('L') => FormAction::LangToggle,
                _ => FormAction::Unhandled,
            }
        } else {
            // Dropdown open — navigate pending selection
            match key.code {
                KeyCode::Up => {
                    if self.pending_idx > 0 { self.pending_idx -= 1; }
                    FormAction::Consumed
                }
                KeyCode::Down => {
                    let max = self.options.len().saturating_sub(1);
                    if self.pending_idx < max { self.pending_idx += 1; }
                    FormAction::Consumed
                }
                KeyCode::Left | KeyCode::Esc   => { self.close_reject(); FormAction::Consumed      }
                KeyCode::Right | KeyCode::Enter => { self.close_accept(); FormAction::ValueChanged  }
                // Swallow everything else while dropdown is open
                _ => FormAction::Consumed,
            }
        }
    }

    fn click_overlay(&mut self, col: u16, row: u16, available: Rect) -> bool {
        if !self.is_open {
            // Click on the field → open dropdown
            if let Some(r) = self.rect {
                if col >= r.x && col < r.right() && row >= r.y && row < r.bottom() {
                    self.open();
                    return true;
                }
            }
            return false;
        }

        // Dropdown is open — check if click landed on an item
        if let Some(dropdown) = self.dropdown_rect(available) {
            if col >= dropdown.x && col < dropdown.right()
                && row > dropdown.y && row < dropdown.bottom()
            {
                let item_row = (row - dropdown.y - 1) as usize;
                if item_row < self.options.len() {
                    self.pending_idx = item_row;
                    self.close_accept();
                    return true;
                }
            }
        }
        // Click outside → close without change
        self.close_reject();
        false
    }
}
