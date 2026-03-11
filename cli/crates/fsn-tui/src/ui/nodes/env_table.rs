// EnvTableNode — editable environment-variable widget with a 3-field editor row.
//
// Design Pattern: Editor + List
//   The active row is always shown at the top as 3 bordered input boxes
//   (KEY | VALUE | COMMENT).  All other rows are listed compactly below.
//
// Navigation:
//   Tab          — move to next column; at col 2: FocusNext (leave table)
//   Shift+Tab    — move to prev column; at col 0: FocusPrev
//   ↑ / ↓        — move between rows
//   Down on last row — adds a new empty row
//   Enter        — add new row after current, go to col 0
//   Ctrl+N       — add new empty row at end
//   Ctrl+D       — delete current row (keeps at least one row)
//   Ctrl+← / →  — tab navigation (TabPrev / TabNext)
//   Backspace    — delete char before cursor in active cell
//   Delete       — delete char after cursor in active cell
//   Esc          — FormAction::Cancel
//
// value() serialization: "KEY=value\n..." for each row with a non-empty key.
// Comments are UI-only and not part of the serialized value.
//
// Layout (inside the outer block):
//   ┌─ KEY ─────────────┐ ┌─ VALUE ──────────────────┐ ┌─ COMMENT ───────┐
//   │ DATABASE_HOST      │ │ localhost█               │ │ DB hostname     │
//   └────────────────────┘ └──────────────────────────┘ └─────────────────┘
//   OTHER_KEY = other_value
//   ...

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use crate::ui::render_ctx::RenderCtx;

use crate::app::Lang;
use crate::ui::form_node::{handle_form_nav, FormAction, FormNode};

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
        };
        node.rebuild_cache();
        node
    }

    pub fn hint(mut self, k: &'static str) -> Self { self.hint_key = Some(k); self }

    pub fn rows(mut self, n: u16) -> Self {
        self.visible_rows = n.max(1);
        self
    }

    // col/min_w accepted but ignored — EnvTable always fills full width.
    pub fn col(self, _n: u8)    -> Self { self }
    pub fn min_w(self, _n: u16) -> Self { self }

    // ── Internal helpers ───────────────────────────────────────────────────

    fn rebuild_cache(&mut self) {
        let mut lines: Vec<String> = Vec::new();
        for r in &self.rows {
            if r[0].trim().is_empty() { continue; }
            if !r[2].trim().is_empty() {
                lines.push(format!("# {}", r[2].trim()));
            }
            lines.push(format!("{}={}", r[0].trim(), r[1].trim()));
        }
        self.cache = lines.join("\n");
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

    fn add_row_at_end(&mut self) {
        self.rows.push(["".to_string(), "".to_string(), "".to_string()]);
        self.cur_row = self.rows.len() - 1;
        self.cur_col = 0;
        self.cur_pos = 0;
        self.dirty = true;
        self.rebuild_cache();
    }

    /// Delete the current row. Keeps at least one (empty) row in the table.
    fn delete_current_row(&mut self) {
        if self.rows.len() == 1 {
            // Clear the only row instead of removing it.
            self.rows[0] = ["".to_string(), "".to_string(), "".to_string()];
            self.cur_col = 0;
            self.cur_pos = 0;
        } else {
            self.rows.remove(self.cur_row);
            if self.cur_row >= self.rows.len() {
                self.cur_row = self.rows.len() - 1;
            }
            self.cur_col = self.cur_col.min(2);
            let cell_len = self.rows[self.cur_row][self.cur_col].len();
            self.cur_pos = self.cur_pos.min(cell_len);
        }
        self.dirty = true;
        self.rebuild_cache();
    }

    fn update_scroll(&mut self) {
        // In the editor+list layout, scroll_offset controls which other rows
        // (all rows except cur_row) are visible in the list below the editor.
        // We ensure the rows adjacent to cur_row remain visible.
        let n = (self.visible_rows as usize).saturating_sub(1); // rows available below editor
        if n == 0 { return; }

        // Virtual index: in the "other rows" list (excluding cur_row), the row
        // just before cur_row is at virtual index max(0, cur_row - 1).
        let target = if self.cur_row > 0 { self.cur_row - 1 } else { 0 };
        if target < self.scroll_offset {
            self.scroll_offset = target;
        } else if target >= self.scroll_offset + n {
            self.scroll_offset = target + 1 - n;
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
        let mut pending_comment = String::new();
        for line in v.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            if let Some(comment) = line.strip_prefix('#') {
                // `# comment` line — attach to the next KEY=value row.
                pending_comment = comment.trim().to_string();
                continue;
            }
            let (k, rest) = line.split_once('=').unwrap_or((line, ""));
            self.rows.push([k.trim().to_string(), rest.trim().to_string(), pending_comment.clone()]);
            pending_comment.clear();
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

    /// 3 (editor boxes) + (visible_rows - 1) (other-rows list) + 2 (block borders) + 1 (hint).
    fn preferred_height(&self) -> u16 { self.visible_rows + 5 }

    fn render(&mut self, f: &mut RenderCtx<'_>, area: Rect, focused: bool, lang: Lang) {
        if focused { self.update_scroll(); }

        // Split into block area and hint line.
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);
        let [block_area, hint_area] = [chunks[0], chunks[1]];

        // ── Outer block ──────────────────────────────────────────────────────
        let label_text  = crate::i18n::t(lang, self.label_key);
        // Show row counter in title so the user always knows how many rows exist.
        let title_text  = if focused && self.rows.len() > 1 {
            format!(" {} [{}/{}] ", label_text, self.cur_row + 1, self.rows.len())
        } else {
            format!(" {} ", label_text)
        };
        let label_style = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let outer_border = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .title(Line::from(Span::styled(title_text, label_style)))
            .borders(Borders::ALL)
            .border_style(outer_border);

        let inner = block.inner(block_area);
        f.render_widget(block, block_area);

        if inner.height == 0 { return; }

        if focused {
            // ── Focused: 3-box editor (current row) + compact list (other rows) ──
            //
            // Editor takes exactly 3 rows (top border + content + bottom border).
            const EDITOR_H: u16 = 3;

            // Column widths: KEY(28%) | VALUE(40%) | COMMENT(rest)
            let key_w = (inner.width * 28 / 100).max(5);
            let val_w = (inner.width * 40 / 100).max(8);
            let com_w = inner.width.saturating_sub(key_w).saturating_sub(val_w);

            let col_rects = [
                Rect { x: inner.x,                y: inner.y, width: key_w, height: EDITOR_H.min(inner.height) },
                Rect { x: inner.x + key_w,         y: inner.y, width: val_w, height: EDITOR_H.min(inner.height) },
                Rect { x: inner.x + key_w + val_w, y: inner.y, width: com_w, height: EDITOR_H.min(inner.height) },
            ];
            let col_names = ["KEY", "VALUE", "COMMENT"];

            for (ci, &col_rect) in col_rects.iter().enumerate() {
                if col_rect.width == 0 { continue; }

                let cell_val  = self.rows.get(self.cur_row).map(|r| r[ci].as_str()).unwrap_or("");
                let is_active = ci == self.cur_col;

                let (border_style, title_style) = if is_active {
                    (Style::default().fg(Color::Cyan), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                } else {
                    (Style::default().fg(Color::DarkGray), Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD))
                };

                let cell_block = Block::default()
                    .title(Span::styled(format!(" {} ", col_names[ci]), title_style))
                    .borders(Borders::ALL)
                    .border_style(border_style);
                let cell_inner = cell_block.inner(col_rect);
                f.render_widget(cell_block, col_rect);

                if cell_inner.height == 0 { continue; }

                let pos    = self.cur_pos.min(cell_val.len());
                let before = &cell_val[..pos];
                let after  = &cell_val[pos..];
                let line = if is_active {
                    Line::from(vec![
                        Span::styled(before.to_string(), Style::default().fg(Color::White)),
                        Span::styled("█",               Style::default().fg(Color::Cyan)),
                        Span::styled(after.to_string(),  Style::default().fg(Color::White)),
                    ])
                } else {
                    let style = if cell_val.is_empty() {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    Line::from(Span::styled(cell_val.to_string(), style))
                };
                f.render_stateful_widget(Paragraph::new(line), cell_inner, &mut ParagraphState::new());
            }

            // Other rows: compact list below the editor
            let list_y = inner.y + EDITOR_H;
            if list_y < inner.bottom() {
                let n_slots = (inner.bottom() - list_y) as usize;
                let mut slot = 0usize;
                for row_idx in 0..self.rows.len() {
                    if row_idx == self.cur_row { continue; }
                    let virtual_idx = if row_idx < self.cur_row { row_idx } else { row_idx - 1 };
                    if virtual_idx < self.scroll_offset { continue; }
                    if slot >= n_slots { break; }
                    render_compact_row(f, &self.rows[row_idx], inner.x, list_y + slot as u16, inner.width);
                    slot += 1;
                }
            }
        } else {
            // ── Unfocused: compact list of ALL rows — data is always visible ──
            //
            // This is the key UX fix: when focus leaves the env table, every
            // row remains readable as a compact "KEY = value" entry so the user
            // can confirm their input without re-focusing the field.
            let n_slots = inner.height as usize;
            for (i, row) in self.rows.iter().enumerate() {
                if i >= n_slots { break; }
                render_compact_row(f, row, inner.x, inner.y + i as u16, inner.width);
            }
        }

        // ── Hint bar ─────────────────────────────────────────────────────────
        let hint_text = if focused {
            "Tab: next col   ↑/↓: rows   Enter: new row   Ctrl+D: delete"
        } else if let Some(hk) = self.hint_key {
            crate::i18n::t(lang, hk)
        } else {
            ""
        };
        if !hint_text.is_empty() {
            f.render_stateful_widget(
                Paragraph::new(Line::from(Span::styled(
                    hint_text.to_string(),
                    Style::default().fg(Color::DarkGray),
                ))),
                hint_area,
                &mut ParagraphState::new(),
            );
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        // Ctrl+S=Submit, Ctrl+←/→=TabPrev/Next — consistent across all nodes.
        if let Some(nav) = handle_form_nav(key) { return nav; }

        use KeyModifiers as KM;

        match key.code {
            // Form-level escape
            KeyCode::Esc => return FormAction::Cancel,

            // Tab = column navigation within the table.
            // At the last column, Tab exits the table and advances to the next field.
            // At the first column, BackTab exits backwards.
            // Using FocusNext/Prev (not TabNext/Prev) so the user stays on the same
            // form tab and can reach other fields after the env table.
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

            // Ctrl+N: add new row at end
            KeyCode::Char('n') if key.modifiers.contains(KM::CONTROL) => {
                self.add_row_at_end();
                return FormAction::ValueChanged;
            }

            // Ctrl+D: delete current row (keeps at least one empty row)
            KeyCode::Char('d') if key.modifiers.contains(KM::CONTROL) => {
                self.delete_current_row();
                return FormAction::ValueChanged;
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

// ── Module-level helpers ──────────────────────────────────────────────────────

/// Render one env row as a compact `  KEY = value` line.
/// Used by both the focused-mode "other rows" list and the unfocused compact view.
fn render_compact_row(
    f:     &mut RenderCtx<'_>,
    row:   &[String; 3],
    x:     u16,
    y:     u16,
    width: u16,
) {
    let key = row[0].as_str();
    let val = row[1].as_str();
    let line = if key.is_empty() {
        Line::from(Span::styled("  —", Style::default().fg(Color::DarkGray)))
    } else {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(key.to_string(),    Style::default().fg(Color::White)),
            Span::styled(" = ".to_string(),  Style::default().fg(Color::DarkGray)),
            Span::styled(val.to_string(),    Style::default().fg(Color::DarkGray)),
        ])
    };
    f.render_stateful_widget(
        Paragraph::new(line),
        Rect { x, y, width, height: 1 },
        &mut ParagraphState::new(),
    );
}
