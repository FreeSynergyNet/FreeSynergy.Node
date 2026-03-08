// EnvTableNode — editable key-value-comment table for environment variables.
//
// Analogous to a spreadsheet with 3 fixed columns: Key | Value | Comment.
// Comments are displayed in the UI but not included in the serialized value.
//
// Navigation:
//   Tab          — move to next column; at col 2: FocusNext (leave table)
//   Shift+Tab    — move to prev column; at col 0: FocusPrev
//   ↑ / ↓        — move between rows
//   Down on last row — adds a new empty row
//   Enter        — add new row after current, go to col 0
//   Ctrl+Enter   — FormAction::Submit (form-level)
//   Ctrl+← / →  — tab navigation (TabPrev / TabNext)
//   Backspace    — delete char before cursor in active cell
//   Delete       — delete char after cursor in active cell
//   Esc          — FormAction::Cancel
//
// value() serialization: "KEY=value\n..." for each row with a non-empty key.
// Comments are UI-only and not part of the serialized value.

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

const DEFAULT_VISIBLE_ROWS: u16 = 3;

#[derive(Debug)]
pub struct EnvTableNode {
    pub key:          &'static str,
    pub label_key:    &'static str,
    pub hint_key:     Option<&'static str>,
    pub tab:          usize,
    /// Rows: each is [key, value, comment]. Always has at least one entry.
    rows:             Vec<[String; 3]>,
    cur_row:          usize,
    cur_col:          usize,
    /// Byte position within the active cell.
    cur_pos:          usize,
    scroll_offset:    usize,
    pub visible_rows: u16,
    /// Cache: serialized form — always `KEY=value\n...` for non-empty-key rows.
    cache:            String,
    pub dirty:        bool,
    rect:             Option<Rect>,
}

impl EnvTableNode {
    pub fn new(key: &'static str, label_key: &'static str, tab: usize) -> Self {
        let mut node = Self {
            key,
            label_key,
            hint_key: None,
            tab,
            rows: vec![["".to_string(), "".to_string(), "".to_string()]],
            cur_row: 0,
            cur_col: 0,
            cur_pos: 0,
            scroll_offset: 0,
            visible_rows: DEFAULT_VISIBLE_ROWS,
            cache: String::new(),
            dirty: false,
            rect: None,
        };
        node.rebuild_cache();
        node
    }

    pub fn hint(mut self, k: &'static str) -> Self { self.hint_key = Some(k); self }

    pub fn rows(mut self, n: u16) -> Self {
        self.visible_rows = n.max(1);
        self
    }

    // ── Internal helpers ───────────────────────────────────────────────────

    fn rebuild_cache(&mut self) {
        self.cache = self.rows.iter()
            .filter(|r| !r[0].trim().is_empty())
            .map(|r| format!("{}={}", r[0].trim(), r[1].trim()))
            .collect::<Vec<_>>()
            .join("\n");
    }

    fn insert_char(&mut self, c: char) {
        let pos = self.cur_pos;
        self.rows[self.cur_row][self.cur_col].insert(pos, c);
        self.cur_pos += c.len_utf8();
        self.dirty = true;
        self.rebuild_cache();
    }

    fn backspace(&mut self) {
        let pos = self.cur_pos;
        if pos > 0 {
            let cell = &mut self.rows[self.cur_row][self.cur_col];
            let prev = cell[..pos].char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            cell.remove(prev);
            self.cur_pos = prev;
            self.dirty = true;
            self.rebuild_cache();
        }
    }

    fn delete_char(&mut self) {
        let pos = self.cur_pos;
        let cell_len = self.rows[self.cur_row][self.cur_col].len();
        if pos < cell_len {
            let next = {
                let cell = &self.rows[self.cur_row][self.cur_col];
                cell[pos..].chars().next().map(|c| pos + c.len_utf8()).unwrap_or(pos)
            };
            self.rows[self.cur_row][self.cur_col].drain(pos..next);
            self.dirty = true;
            self.rebuild_cache();
        }
    }

    fn cursor_left(&mut self) {
        if self.cur_pos > 0 {
            let cell = &self.rows[self.cur_row][self.cur_col];
            self.cur_pos = cell[..self.cur_pos]
                .char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
        }
    }

    fn cursor_right(&mut self) {
        let cell = &self.rows[self.cur_row][self.cur_col];
        if self.cur_pos < cell.len() {
            let next = cell[self.cur_pos..].chars().next()
                .map(|c| self.cur_pos + c.len_utf8()).unwrap_or(self.cur_pos);
            self.cur_pos = next;
        }
    }

    fn move_to_row(&mut self, row: usize) {
        self.cur_row = row;
        let cell_len = self.rows[self.cur_row][self.cur_col].len();
        self.cur_pos = self.cur_pos.min(cell_len);
    }

    fn add_row_after_current(&mut self) {
        let insert_at = self.cur_row + 1;
        self.rows.insert(
            insert_at,
            ["".to_string(), "".to_string(), "".to_string()],
        );
        self.cur_row = insert_at;
        self.cur_col = 0;
        self.cur_pos = 0;
    }

    fn update_scroll(&mut self) {
        let n = self.visible_rows as usize;
        if self.cur_row >= self.scroll_offset + n {
            self.scroll_offset = self.cur_row + 1 - n;
        } else if self.cur_row < self.scroll_offset {
            self.scroll_offset = self.cur_row;
        }
    }
}

impl FormNode for EnvTableNode {
    fn key(&self)       -> &'static str         { self.key }
    fn label_key(&self) -> &'static str         { self.label_key }
    fn hint_key(&self)  -> Option<&'static str> { self.hint_key }
    fn tab(&self)       -> usize                { self.tab }
    fn required(&self)  -> bool                 { false }

    fn value(&self)          -> &str { &self.cache }
    fn effective_value(&self) -> &str { &self.cache }

    fn set_value(&mut self, v: &str) {
        self.rows.clear();
        for line in v.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let (k, rest) = line.split_once('=').unwrap_or((line, ""));
            self.rows.push([k.trim().to_string(), rest.trim().to_string(), String::new()]);
        }
        if self.rows.is_empty() {
            self.rows.push(["".to_string(), "".to_string(), "".to_string()]);
        }
        self.cur_row = self.cur_row.min(self.rows.len() - 1);
        self.cur_pos = 0;
        self.rebuild_cache();
    }

    fn is_dirty(&self)       -> bool { self.dirty }
    fn set_dirty(&mut self, v: bool) { self.dirty = v; }

    fn set_rect(&mut self, r: Rect)     { self.rect = Some(r); }
    fn last_rect(&self) -> Option<Rect> { self.rect }

    /// Block(borders=2 + header=1 + rows=N) + hint=1.
    fn preferred_height(&self) -> u16 { self.visible_rows + 4 }

    fn render(&mut self, f: &mut Frame, area: Rect, focused: bool, lang: Lang) {
        self.set_rect(area);
        if focused { self.update_scroll(); }

        let [block_area, hint_area] = {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(area);
            [chunks[0], chunks[1]]
        };

        // Outer block
        let label_text   = crate::i18n::t(lang, self.label_key);
        let label_style  = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .title(Line::from(vec![
                Span::styled(format!(" {} ", label_text), label_style),
            ]))
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(block_area);
        f.render_widget(block, block_area);

        if inner.height == 0 { return; }

        // Column proportions: Key(30%) | Value(35%) | Comment(35%)
        let key_w  = (inner.width * 30 / 100).max(6);
        let val_w  = (inner.width * 35 / 100).max(8);
        let com_w  = inner.width.saturating_sub(key_w).saturating_sub(val_w);

        let col_starts = [inner.x, inner.x + key_w, inner.x + key_w + val_w];
        let col_widths = [key_w, val_w, com_w];

        let header_names = ["KEY", "VALUE", "COMMENT"];

        // Header row
        let header_y = inner.y;
        if header_y < inner.bottom() {
            for ci in 0..3 {
                let style = if focused && ci == self.cur_col {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)
                };
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(header_names[ci], style))),
                    Rect { x: col_starts[ci], y: header_y, width: col_widths[ci], height: 1 },
                );
            }
        }

        // Data rows
        let n_visible = self.visible_rows as usize;
        for slot in 0..n_visible {
            let row_idx = self.scroll_offset + slot;
            let row_y   = inner.y + 1 + slot as u16;
            if row_y >= inner.bottom() { break; }

            let is_active_row = focused && row_idx == self.cur_row;
            let row_data = self.rows.get(row_idx);

            for ci in 0..3usize {
                let cell_rect = Rect {
                    x: col_starts[ci], y: row_y,
                    width: col_widths[ci], height: 1,
                };
                let cell_val  = row_data.map(|r| r[ci].as_str()).unwrap_or("");
                let is_active = is_active_row && ci == self.cur_col;

                let line = if is_active {
                    let pos    = self.cur_pos.min(cell_val.len());
                    let before = &cell_val[..pos];
                    let after  = &cell_val[pos..];
                    Line::from(vec![
                        Span::styled(before.to_string(), Style::default().fg(Color::White)),
                        Span::styled("█",               Style::default().fg(Color::Cyan)),
                        Span::styled(after.to_string(),  Style::default().fg(Color::White)),
                    ])
                } else {
                    let style = if is_active_row {
                        Style::default().fg(Color::White)
                    } else if cell_val.is_empty() {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    Line::from(Span::styled(cell_val.to_string(), style))
                };
                f.render_widget(Paragraph::new(line), cell_rect);
            }
        }

        // Hint
        if let Some(hk) = self.hint_key {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    crate::i18n::t(lang, hk),
                    Style::default().fg(Color::DarkGray),
                ))),
                hint_area,
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        use KeyModifiers as KM;

        match key.code {
            // Form-level navigation
            KeyCode::Esc     => return FormAction::Cancel,
            KeyCode::Left  if key.modifiers.contains(KM::CONTROL) => return FormAction::TabPrev,
            KeyCode::Right if key.modifiers.contains(KM::CONTROL) => return FormAction::TabNext,
            KeyCode::Enter if key.modifiers.contains(KM::CONTROL) => return FormAction::Submit,

            // Column navigation via Tab
            KeyCode::Tab => {
                if self.cur_col < 2 {
                    self.cur_col += 1;
                    let cell_len = self.rows[self.cur_row][self.cur_col].len();
                    self.cur_pos = self.cur_pos.min(cell_len);
                    return FormAction::Consumed;
                } else {
                    return FormAction::FocusNext;
                }
            }
            KeyCode::BackTab => {
                if self.cur_col > 0 {
                    self.cur_col -= 1;
                    let cell_len = self.rows[self.cur_row][self.cur_col].len();
                    self.cur_pos = self.cur_pos.min(cell_len);
                    return FormAction::Consumed;
                } else {
                    return FormAction::FocusPrev;
                }
            }

            // Row navigation
            KeyCode::Up => {
                if self.cur_row > 0 {
                    self.move_to_row(self.cur_row - 1);
                }
                return FormAction::Consumed;
            }
            KeyCode::Down => {
                if self.cur_row < self.rows.len() - 1 {
                    self.move_to_row(self.cur_row + 1);
                } else {
                    // Add new row at end
                    self.rows.push(["".to_string(), "".to_string(), "".to_string()]);
                    self.cur_row = self.rows.len() - 1;
                    self.cur_col = 0;
                    self.cur_pos = 0;
                }
                return FormAction::Consumed;
            }

            // Enter: add new row after current
            KeyCode::Enter => {
                self.add_row_after_current();
                return FormAction::Consumed;
            }

            // Cursor movement within cell
            KeyCode::Left  => { self.cursor_left();             return FormAction::Consumed; }
            KeyCode::Right => { self.cursor_right();            return FormAction::Consumed; }
            KeyCode::Home  => { self.cur_pos = 0;               return FormAction::Consumed; }
            KeyCode::End   => {
                self.cur_pos = self.rows[self.cur_row][self.cur_col].len();
                return FormAction::Consumed;
            }

            // Editing
            KeyCode::Backspace => { self.backspace();   return FormAction::ValueChanged; }
            KeyCode::Delete    => { self.delete_char(); return FormAction::ValueChanged; }

            KeyCode::Char(c) if !key.modifiers.contains(KM::CONTROL) => {
                self.insert_char(c);
                return FormAction::ValueChanged;
            }

            _ => {}
        }

        FormAction::Unhandled
    }
}
