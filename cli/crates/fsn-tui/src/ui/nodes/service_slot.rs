// Service slot node — categorized service picker with type-filter.
//
// Design Pattern: Composite — type-filter cycle + categorized item list combined
// in one popup. The node owns both the filter state and the item cursor.
//
// UX: focused field shows current value + "▼" hint (same as SelectInputNode).
//     ↓/↑/Enter opens a centered popup.
//     Inside popup: filter row (Left/Right/Enter cycle type), item list (↑↓ navigate).
//     Enter/→ on item = confirm. Esc = close without change. Mouse supported.

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use crate::app::Lang;
use crate::ui::form_node::{handle_form_nav, FormAction, FormNode};
use crate::ui::render_ctx::RenderCtx;

// ── Category ──────────────────────────────────────────────────────────────────

/// Which category a service entry belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotCategory {
    Configured,
    Available,
    Store,
}

// ── SlotEntry ─────────────────────────────────────────────────────────────────

/// One item in the service slot popup.
#[derive(Debug, Clone)]
pub struct SlotEntry {
    /// Human-readable label shown in the popup.
    pub display:      String,
    /// Encoded value stored in the form field (see module-level value encoding docs).
    pub value:        String,
    pub category:     SlotCategory,
    /// Short service type tag, e.g. "iam", "proxy", "" for external.
    pub service_type: String,
}

impl SlotEntry {
    /// A locally configured service instance.
    pub fn configured(name: &str, svc_type: &str) -> Self {
        Self {
            display:      name.to_string(),
            value:        name.to_string(),
            category:     SlotCategory::Configured,
            service_type: svc_type.to_string(),
        }
    }

    /// A locally available (compiled-in) service class not yet deployed.
    /// `value` = `"new:{class}"`.
    pub fn available(class: &str, display: &str, svc_type: &str) -> Self {
        Self {
            display:      display.to_string(),
            value:        format!("new:{}", class),
            category:     SlotCategory::Available,
            service_type: svc_type.to_string(),
        }
    }

    /// A module available in the store (download required).
    /// `value` = `"store:{id}"`.
    pub fn store_module(id: &str, display: &str, svc_type: &str) -> Self {
        Self {
            display:      display.to_string(),
            value:        format!("store:{}", id),
            category:     SlotCategory::Store,
            service_type: svc_type.to_string(),
        }
    }

    /// An externally hosted service (no local deployment).
    #[allow(dead_code)]
    pub fn external() -> Self {
        Self {
            display:      "External service".to_string(),
            value:        "external".to_string(),
            category:     SlotCategory::Available,
            service_type: String::new(),
        }
    }
}

// ── ServiceSlotNode ───────────────────────────────────────────────────────────

/// Service slot form field — composite picker with type filter + categorized list.
#[derive(Debug)]
pub struct ServiceSlotNode {
    pub key:        &'static str,
    pub label_key:  &'static str,
    pub hint_key:   Option<&'static str>,
    pub tab:        usize,
    pub required:   bool,
    pub value:      String,
    pub col_span:   u8,
    pub min_width:  u16,

    entries:         Vec<SlotEntry>,
    /// Filter options: first is always "all", followed by unique service_type strings.
    type_options:    Vec<String>,
    type_filter_idx: usize,
    /// When a specific type is pre-set, hide the filter row.
    show_filter:     bool,

    pub is_open:    bool,
    /// Whether keyboard focus is on the type-filter row (not the item list).
    on_filter_row:  bool,
    /// Index within `visible_entries()` — the item the cursor is on.
    cursor:         usize,

    // Populated during render_overlay(), used for mouse hit-testing.
    rendered_rect:   Option<Rect>,
    filter_row_rect: Option<Rect>,
    /// Maps (visible item index → rendered Rect).
    item_rects:      Vec<(usize, Rect)>,
}

impl ServiceSlotNode {
    /// Create a new service slot node.
    ///
    /// `default_type` pre-selects a type filter (e.g. "iam").
    /// Pass `""` to start on "all".
    pub fn new(
        key:          &'static str,
        label_key:    &'static str,
        tab:          usize,
        required:     bool,
        entries:      Vec<SlotEntry>,
        default_type: &str,
    ) -> Self {
        // Build type_options: ["all"] + unique service_type values (non-empty)
        let mut seen = std::collections::HashSet::new();
        let mut type_options = vec!["all".to_string()];
        for e in &entries {
            if !e.service_type.is_empty() && seen.insert(e.service_type.clone()) {
                type_options.push(e.service_type.clone());
            }
        }

        let type_filter_idx = if default_type.is_empty() {
            0
        } else {
            type_options.iter().position(|t| t == default_type).unwrap_or(0)
        };

        let show_filter = default_type.is_empty();

        Self {
            key, label_key, hint_key: None, tab, required,
            value: String::new(),
            col_span: 12, min_width: 0,
            entries, type_options, type_filter_idx,
            show_filter,
            is_open: false, on_filter_row: false, cursor: 0,
            rendered_rect: None, filter_row_rect: None, item_rects: Vec::new(),
        }
    }

    // ── Builder helpers ────────────────────────────────────────────────────

    pub fn hint(mut self, k: &'static str) -> Self { self.hint_key = Some(k); self }
    pub fn col(mut self, n: u8)             -> Self { self.col_span = n.min(12).max(1); self }
    pub fn min_w(mut self, n: u16)          -> Self { self.min_width = n; self }

    pub fn with_value(mut self, v: &str) -> Self {
        self.value = v.to_string();
        self
    }

    // ── Internal helpers ───────────────────────────────────────────────────

    /// Entries visible under the current type filter (cloned for ownership).
    fn visible_entries(&self) -> Vec<SlotEntry> {
        let filter = &self.type_options[self.type_filter_idx];
        if filter == "all" {
            self.entries.clone()
        } else {
            self.entries.iter()
                .filter(|e| &e.service_type == filter || e.service_type.is_empty())
                .cloned()
                .collect()
        }
    }

    fn cycle_filter_right(&mut self) {
        let n = self.type_options.len();
        self.type_filter_idx = (self.type_filter_idx + 1) % n;
        self.cursor = 0;
    }

    fn cycle_filter_left(&mut self) {
        let n = self.type_options.len();
        self.type_filter_idx = (self.type_filter_idx + n - 1) % n;
        self.cursor = 0;
    }

    /// Human-readable label for the current value shown in the closed field.
    fn display_value(&self) -> String {
        if self.value.is_empty() {
            return "—".to_string();
        }
        if self.value == "external" {
            return "External service".to_string();
        }
        if let Some(class) = self.value.strip_prefix("new:") {
            let short = class.split('/').last().unwrap_or(class);
            return format!("+ {}", short);
        }
        if let Some(id) = self.value.strip_prefix("store:") {
            let short = id.split('/').last().unwrap_or(id);
            return format!("↓ {}", short);
        }
        // Configured service instance — return its name directly.
        self.value.clone()
    }

    /// Open the popup and position cursor at the currently selected entry.
    fn open(&mut self) {
        let visible = self.visible_entries();
        self.cursor = visible.iter()
            .position(|e| e.value == self.value)
            .unwrap_or(0);
        self.on_filter_row = false;
        self.is_open = true;
    }

    /// Confirm the item at `cursor`, close the popup, return AcceptAndNext.
    fn confirm_current(&mut self) -> FormAction {
        let visible = self.visible_entries();
        if let Some(entry) = visible.get(self.cursor) {
            self.value = entry.value.clone();
        }
        self.is_open = false;
        FormAction::AcceptAndNext
    }

    // ── Popup key handler ──────────────────────────────────────────────────

    fn handle_popup_key(&mut self, key: KeyEvent) -> FormAction {
        let vis_len = self.visible_entries().len();

        match key.code {
            KeyCode::Esc => {
                self.is_open = false;
                FormAction::Consumed
            }
            KeyCode::Up => {
                if self.on_filter_row {
                    // noop — already at top
                } else if self.cursor == 0 {
                    self.on_filter_row = true;
                } else {
                    self.cursor -= 1;
                }
                FormAction::Consumed
            }
            KeyCode::Down => {
                if self.on_filter_row {
                    self.on_filter_row = false;
                } else if vis_len > 0 {
                    self.cursor = (self.cursor + 1).min(vis_len - 1);
                }
                FormAction::Consumed
            }
            KeyCode::Left => {
                if self.on_filter_row {
                    self.cycle_filter_left();
                } else {
                    // Left outside filter = close (cancel)
                    self.is_open = false;
                }
                FormAction::Consumed
            }
            KeyCode::Right => {
                if self.on_filter_row {
                    self.cycle_filter_right();
                } else {
                    return self.confirm_current();
                }
                FormAction::Consumed
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.on_filter_row {
                    self.cycle_filter_right();
                    FormAction::Consumed
                } else {
                    self.confirm_current()
                }
            }
            _ => FormAction::Consumed, // swallow all other keys while popup is open
        }
    }

    // ── Popup rendering ────────────────────────────────────────────────────

    fn render_popup(&mut self, f: &mut RenderCtx<'_>, lang: Lang) {
        let screen = f.area();
        // Collect owned entries first so we can freely mutate self afterwards.
        let visible: Vec<SlotEntry> = self.visible_entries();
        let n_items = visible.len();

        // Popup sizing: width=52 max, height = items + separators + borders + filter row
        // Reserve: 2 (border) + 1 (filter row) + 1 (filter separator) + 1 (bottom separator) + 1 (external)
        let popup_w  = (52_u16).min(screen.width);
        let content_h = (n_items as u16) + 5; // rough estimate with category separators
        let popup_h  = content_h.min(24).min(screen.height);

        let popup = Rect {
            x:      screen.width.saturating_sub(popup_w) / 2,
            y:      screen.height.saturating_sub(popup_h) / 2,
            width:  popup_w,
            height: popup_h,
        };
        self.rendered_rect = Some(popup);

        let title_text = crate::i18n::t(lang, self.label_key);
        let block = Block::default()
            .title(Span::styled(
                format!(" {} ", title_text),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup);
        f.render_widget(Clear, popup);
        f.render_widget(block, popup);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // Filter row — only shown when no specific type is pre-set
        let items_top = if self.show_filter {
            let filter_rect = Rect { height: 1, ..inner };
            self.filter_row_rect = Some(filter_rect);

            let current_filter = &self.type_options[self.type_filter_idx];
            let filter_style = if self.on_filter_row {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let filter_line = Line::from(vec![
                Span::styled("  Filter: ", Style::default().fg(Color::DarkGray)),
                Span::styled("◀ ", Style::default().fg(Color::Cyan)),
                Span::styled(current_filter.clone(), filter_style),
                Span::styled(" ▶", Style::default().fg(Color::Cyan)),
            ]);
            f.render_stateful_widget(
                Paragraph::new(filter_line),
                filter_rect,
                &mut ParagraphState::new(),
            );
            inner.y + 1
        } else {
            self.filter_row_rect = None;
            inner.y
        };

        // Items area — below the filter row (or at top when filter is hidden)
        if items_top >= inner.bottom() {
            return;
        }
        let items_area = Rect {
            y:      items_top,
            height: inner.bottom().saturating_sub(items_top),
            ..inner
        };

        self.item_rects.clear();
        let mut row_y = items_area.y;
        let mut last_category: Option<SlotCategory> = None;
        let mut visible_idx = 0usize;

        for entry in &visible {
            // Emit category separator when category changes
            let cat = entry.category;
            if last_category != Some(cat) && row_y < items_area.bottom() {
                let sep_label = match cat {
                    SlotCategory::Configured => " ── Services ──",
                    SlotCategory::Available  => " ── Available ──",
                    SlotCategory::Store      => " ── Store ──",
                };
                let sep_rect = Rect { y: row_y, height: 1, ..items_area };
                f.render_stateful_widget(
                    Paragraph::new(Line::from(Span::styled(
                        sep_label,
                        Style::default().fg(Color::DarkGray),
                    ))),
                    sep_rect,
                    &mut ParagraphState::new(),
                );
                row_y += 1;
                last_category = Some(cat);
            }

            if row_y >= items_area.bottom() {
                break;
            }

            let is_cursor = visible_idx == self.cursor;
            let marker = if is_cursor { "◉ " } else { "○ " };

            let (cat_icon, cat_color) = match cat {
                SlotCategory::Configured => ("✓", Color::Green),
                SlotCategory::Available  => ("+", Color::Yellow),
                SlotCategory::Store      => ("↓", Color::Blue),
            };

            let item_rect = Rect { y: row_y, height: 1, ..items_area };
            self.item_rects.push((visible_idx, item_rect));

            let item_style = if is_cursor {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            // Build item line: "  ◉ name  ·  type  ·  icon"
            // Columns: name (left-aligned, truncated) · service_type (DarkGray) · icon (colored)
            // Separator "  ·  " is DarkGray. If service_type is empty, skip the · type part.
            let sep_style    = Style::default().fg(Color::DarkGray);
            let type_style   = Style::default().fg(Color::DarkGray);
            let icon_style   = Style::default().fg(cat_color);

            // Reserve space: "  ◉ " (4) + "  ·  icon" (7) + optional "  ·  type" (5 + type.len())
            let type_reserve = if entry.service_type.is_empty() { 0 } else { 5 + entry.service_type.len() };
            let base_reserve = 4 + 7 + type_reserve;
            let label_width  = (items_area.width as usize).saturating_sub(base_reserve).max(1);
            let display_truncated = if entry.display.len() > label_width {
                format!("{:.width$}", entry.display, width = label_width)
            } else {
                entry.display.clone()
            };

            let mut spans = vec![
                Span::styled(format!("  {}{}", marker, display_truncated), item_style),
            ];
            if !entry.service_type.is_empty() {
                spans.push(Span::styled("  ·  ", sep_style));
                spans.push(Span::styled(entry.service_type.clone(), type_style));
            }
            spans.push(Span::styled("  ·  ", sep_style));
            spans.push(Span::styled(cat_icon, icon_style));

            let line = Line::from(spans);
            f.render_stateful_widget(
                Paragraph::new(line),
                item_rect,
                &mut ParagraphState::new(),
            );

            row_y += 1;
            visible_idx += 1;
        }

        // Hint line at the bottom of the popup
        if row_y < items_area.bottom() {
            let hint_rect = Rect {
                y:      items_area.bottom().saturating_sub(1),
                height: 1,
                ..items_area
            };
            let hint = "↑↓=Navigate  Enter=Select  Esc=Cancel";
            f.render_stateful_widget(
                Paragraph::new(Line::from(Span::styled(
                    hint,
                    Style::default().fg(Color::DarkGray),
                ))),
                hint_rect,
                &mut ParagraphState::new(),
            );
        }
    }
}

// ── FormNode impl ─────────────────────────────────────────────────────────────

impl FormNode for ServiceSlotNode {
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

    fn is_focusable(&self) -> bool { true }
    fn preferred_height(&self) -> u16 { 4 }

    fn is_filled(&self) -> bool { !self.value.is_empty() }

    fn render(&mut self, f: &mut RenderCtx<'_>, area: Rect, focused: bool, lang: Lang) {
        // Layout: 3-row input box + 1-row hint (same as SelectInputNode)
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(1)])
            .split(area);

        let label_text = crate::i18n::t(lang, self.label_key);
        let req_suffix = if self.required { " *" } else { "" };
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

        let display = self.display_value();
        let input_line = if focused {
            Line::from(vec![
                Span::styled(display, Style::default().fg(Color::White)),
                Span::styled(" ▼", Style::default().fg(Color::Cyan)),
            ])
        } else {
            Line::from(Span::styled(display, Style::default().fg(Color::White)))
        };

        f.render_stateful_widget(
            Paragraph::new(input_line)
                .block(Block::default().borders(Borders::ALL).border_style(border_style).title(title)),
            rows[0],
            &mut ParagraphState::new(),
        );

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

    fn render_overlay(&mut self, f: &mut RenderCtx<'_>, _available: Rect, lang: Lang) {
        if self.is_open {
            self.render_popup(f, lang);
        }
    }

    fn has_open_popup(&self) -> bool { self.is_open }

    fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        if self.is_open {
            return self.handle_popup_key(key);
        }

        // Check global form nav (Ctrl+S, Ctrl+←/→) first.
        if let Some(nav) = handle_form_nav(key) { return nav; }

        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Enter => {
                self.open();
                FormAction::Consumed
            }
            KeyCode::Tab     => FormAction::FocusNext,
            KeyCode::BackTab => FormAction::FocusPrev,
            KeyCode::Esc     => FormAction::Cancel,
            KeyCode::Char('l') | KeyCode::Char('L') => FormAction::LangToggle,
            _ => FormAction::Unhandled,
        }
    }

    fn handle_mouse(&mut self, event: MouseEvent, _area: Rect) -> FormAction {
        if event.kind == MouseEventKind::Down(MouseButton::Left) {
            self.open();
            return FormAction::Consumed;
        }
        FormAction::Unhandled
    }

    fn handle_popup_mouse(&mut self, event: MouseEvent) -> Option<FormAction> {
        let popup_rect = self.rendered_rect?;
        let col = event.column;
        let row = event.row;

        match event.kind {
            MouseEventKind::ScrollUp => {
                if !self.on_filter_row {
                    let vis_len = self.visible_entries().len();
                    if self.cursor > 0 { self.cursor -= 1; }
                    let _ = vis_len;
                }
                return Some(FormAction::Consumed);
            }
            MouseEventKind::ScrollDown => {
                if !self.on_filter_row {
                    let vis_len = self.visible_entries().len();
                    if vis_len > 0 { self.cursor = (self.cursor + 1).min(vis_len - 1); }
                }
                return Some(FormAction::Consumed);
            }
            MouseEventKind::Down(MouseButton::Left) => {}
            _ => return None,
        }

        // Click outside popup — close (preserve selection, no cancel)
        let outside = col < popup_rect.x || col >= popup_rect.right()
            || row < popup_rect.y || row >= popup_rect.bottom();
        if outside {
            self.is_open = false;
            return Some(FormAction::Consumed);
        }

        // Click on filter row
        if let Some(fr) = self.filter_row_rect {
            if row == fr.y {
                self.cycle_filter_right();
                return Some(FormAction::Consumed);
            }
        }

        // Click on item
        let item_rects = self.item_rects.clone();
        for (vis_idx, item_rect) in &item_rects {
            if row == item_rect.y && col >= item_rect.x && col < item_rect.right() {
                self.cursor = *vis_idx;
                return Some(self.confirm_current());
            }
        }

        Some(FormAction::Consumed)
    }
}
