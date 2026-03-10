// TextAreaNode — multi-line text input.
//
// Uses rat-widget's TextArea / TextAreaState (from rat-text) for all rendering
// and text-buffer management:
//   • Multi-line cursor, selection, scrolling handled by the widget
//   • Undo / redo via Ctrl+Z / Ctrl+Y built into TextAreaState
//   • Mouse click-to-position, drag-selection via rat-widget event handling
//
// FormNode wrapper:
//   • Tab=FocusNext (not TabNext) so the user stays on the same form-tab
//     and reaches fields below the textarea.
//   • Enter inserts a newline (normal textarea UX).
//   • `cache: String` mirrors TextAreaState::value() (which returns owned String)
//     to satisfy FormNode::value() -> &str without repeated allocations.
//
// preferred_height = visible_lines + 3  (2 borders + 1 hint row)

use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};
use rat_widget::textarea::{TextArea, TextAreaState, handle_events};
use rat_widget::event::TextOutcome;
use rat_widget::text::HasScreenCursor;

use crate::app::Lang;
use crate::ui::form_node::{handle_form_nav, FormAction, FormNode};
use crate::ui::render_ctx::RenderCtx;

const DEFAULT_ROWS: u16 = 4;

#[derive(Debug)]
pub struct TextAreaNode {
    pub key:           &'static str,
    pub label_key:     &'static str,
    pub hint_key:      Option<&'static str>,
    pub tab:           usize,
    pub required:      bool,
    pub dirty:         bool,
    /// How many text rows are visible in the rendered box.
    pub visible_lines: u16,
    /// rat-widget state: owns buffer, cursor, undo history.
    state:             TextAreaState,
    /// Mirrors state value — satisfies FormNode::value() -> &str without re-allocating.
    cache:             String,
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
            dirty: false,
            visible_lines: DEFAULT_ROWS,
            state: TextAreaState::new(),
            cache: String::new(),
        }
    }

    // ── Builder helpers ────────────────────────────────────────────────────

    pub fn hint(mut self, k: &'static str) -> Self { self.hint_key = Some(k); self }
    pub fn rows(mut self, n: u16)          -> Self { self.visible_lines = n.max(1); self }
    // col/min_w accepted but ignored — TextArea always fills full width.
    pub fn col(self, _n: u8)   -> Self { self }
    pub fn min_w(self, _n: u16) -> Self { self }

    pub fn default_val(mut self, v: &str) -> Self {
        self.state.set_value(v);
        self.cache = v.to_string();
        self
    }

    pub fn pre_filled(mut self, v: &str) -> Self {
        self.state.set_value(v);
        self.cache = v.to_string();
        self.dirty = true;
        self
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

    fn set_value(&mut self, v: &str) {
        self.state.set_value(v);
        self.cache = v.to_string();
    }

    fn is_dirty(&self)       -> bool { self.dirty }
    fn set_dirty(&mut self, v: bool) { self.dirty = v; }

    fn is_filled(&self) -> bool { !self.cache.trim().is_empty() }

    fn preferred_height(&self) -> u16 {
        self.visible_lines + 3 // box(visible_lines + 2 borders) + hint(1)
    }

    fn render(&mut self, f: &mut RenderCtx<'_>, area: Rect, focused: bool, lang: Lang) {
        let box_h = self.visible_lines + 2;
        let rows  = Layout::vertical([Constraint::Length(box_h), Constraint::Length(1)]).split(area);

        let label_text  = crate::i18n::t(lang, self.label_key);
        let req_suffix  = if self.required { " *" } else { "" };
        let label_style = if focused {
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
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Line::from(Span::styled(
                format!(" {}{} ", label_text, req_suffix),
                label_style,
            )));

        self.state.focus.set(focused);

        let widget = TextArea::new()
            .block(block)
            .style(Style::default().fg(Color::White))
            .focus_style(Style::default().fg(Color::White));

        f.render_stateful_widget(widget, rows[0], &mut self.state);

        // Forward cursor position to the frame.
        if focused {
            if let Some(pos) = self.state.screen_cursor() {
                f.set_cursor_position(pos);
            }
        }

        // Hint line
        let hint_text = if let Some(hk) = self.hint_key {
            crate::i18n::t(lang, hk)
        } else {
            crate::i18n::t(lang, "form.textarea.hint")
        };
        f.render_stateful_widget(
            Paragraph::new(Line::from(Span::styled(
                hint_text,
                Style::default().fg(Color::DarkGray),
            ))),
            rows[1],
            &mut ParagraphState::new(),
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        // Ctrl+S=Submit, Ctrl+←=TabPrev, Ctrl+→=TabNext — handled before TextAreaState
        // to prevent them being interpreted as word-navigation shortcuts.
        if let Some(nav) = handle_form_nav(key) { return nav; }

        // Meta keys — TextAreaNode uses FocusNext (not TabNext) so the user stays
        // on the same form tab and reaches fields below the textarea.
        match key.code {
            KeyCode::Tab     => return FormAction::FocusNext,
            KeyCode::BackTab => return FormAction::FocusPrev,
            KeyCode::Esc     => return FormAction::Cancel,
            _ => {}
        }

        // Delegate to rat-widget TextAreaState.
        match handle_events(&mut self.state, true, &Event::Key(key)) {
            TextOutcome::TextChanged => {
                self.cache = self.state.value();
                self.dirty = true;
                FormAction::ValueChanged
            }
            TextOutcome::Unchanged | TextOutcome::Changed => FormAction::Consumed,
            TextOutcome::Continue                         => FormAction::Unhandled,
        }
    }
}
