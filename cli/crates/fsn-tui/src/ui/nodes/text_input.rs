// Text input node — single-line editable field.
//
// Uses rat-widget's TextInput / TextInputState (from rat-text) for all rendering
// and text-buffer management:
//   • Cursor, selection, scrolling handled by the widget
//   • Undo / redo via Ctrl+Z / Ctrl+Y built into TextInputState
//   • Password masking via TextInput::passwd()
//   • Mouse click-to-place-cursor via native rat-widget event handling
//
// FormNode wrapper:
//   • Owns form metadata (key, label, tab, required, default, max_len)
//   • `cache: String` mirrors TextInputState value — avoids Cow<str> lifetime issues
//   • handle_key() intercepts meta keys (Tab, Enter, Esc, Ctrl+S/←/→) before
//     delegating everything else to TextInputState::handle_events()

use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use rat_widget::text_input::{TextInput, TextInputState, handle_events};
use rat_widget::event::TextOutcome;
use rat_widget::text::HasScreenCursor;

use crate::app::Lang;
use crate::ui::form_node::{handle_form_nav, FormAction, FormNode};
use crate::ui::render_ctx::RenderCtx;
use crate::ui::widgets::{node_block, render_hint_opt};

#[derive(Debug)]
pub struct TextInputNode {
    pub key:       &'static str,
    pub label_key: &'static str,
    pub hint_key:  Option<&'static str>,
    pub tab:       usize,
    pub required:  bool,
    pub default:   String,
    pub dirty:     bool,
    pub secret:    bool,
    /// Maximum allowed character count (0 = unlimited).
    pub max_len:   usize,
    pub col_span:  u8,
    pub min_width: u16,
    /// rat-widget state: owns buffer, cursor, undo history.
    state:         TextInputState,
    /// Mirrors state value — satisfies FormNode::value() -> &str without Cow lifetime issues.
    cache:         String,
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
            default: String::new(), dirty: false, secret: false, max_len: 0,
            col_span: 12, min_width: 0,
            state: TextInputState::new(),
            cache: String::new(),
        }
    }

    // ── Builder helpers ────────────────────────────────────────────────────

    pub fn hint(mut self, k: &'static str) -> Self { self.hint_key = Some(k); self }

    pub fn default_val(mut self, v: &str) -> Self {
        self.default = v.to_string();
        self.state.set_value(v);
        self.cache = v.to_string();
        self
    }

    pub fn pre_filled(mut self, v: &str) -> Self {
        self.default = v.to_string();
        self.state.set_value(v);
        self.cache = v.to_string();
        self.dirty = true;
        self
    }

    pub fn secret(mut self) -> Self { self.secret = true; self }

    pub fn max_len(mut self, n: usize) -> Self { self.max_len = n; self }

    pub fn col(mut self, n: u8) -> Self { self.col_span = n.min(12).max(1); self }

    pub fn min_w(mut self, n: u16) -> Self { self.min_width = n; self }
}

impl FormNode for TextInputNode {
    fn key(&self)       -> &'static str         { self.key }
    fn label_key(&self) -> &'static str         { self.label_key }
    fn hint_key(&self)  -> Option<&'static str> { self.hint_key }
    fn tab(&self)       -> usize                { self.tab }
    fn required(&self)  -> bool                 { self.required }
    fn col_span(&self)  -> u8                   { self.col_span }
    fn min_width(&self) -> u16                  { self.min_width }

    fn value(&self) -> &str { &self.cache }

    fn effective_value(&self) -> &str {
        if self.cache.trim().is_empty() && !self.default.is_empty() {
            &self.default
        } else {
            &self.cache
        }
    }

    fn set_value(&mut self, v: &str) {
        self.state.set_value(v);
        self.cache = v.to_string();
    }

    fn is_dirty(&self)       -> bool { self.dirty }
    fn set_dirty(&mut self, v: bool) { self.dirty = v; }

    fn preferred_height(&self) -> u16 { 4 } // input box(3) + hint(1)

    fn render(&mut self, f: &mut RenderCtx<'_>, area: Rect, focused: bool, lang: Lang) {
        let rows = Layout::vertical([Constraint::Length(3), Constraint::Length(1)]).split(area);

        let block = node_block(self.label_key, self.required, focused, lang);

        // Inform the widget about focus so it applies focus_style.
        self.state.focus.set(focused);

        use ratatui::style::{Color, Style};
        let widget = {
            let w = TextInput::new()
                .block(block)
                .style(Style::default().fg(Color::White))
                .focus_style(Style::default().fg(Color::White));
            if self.secret { w.passwd() } else { w }
        };

        f.render_stateful_widget(widget, rows[0], &mut self.state);

        // Forward cursor position to the frame so the terminal places the cursor.
        if focused {
            if let Some(pos) = self.state.screen_cursor() {
                f.set_cursor_position(pos);
            }
        }

        render_hint_opt(f, rows[1], self.hint_key, lang);
    }

    fn handle_mouse(&mut self, event: crossterm::event::MouseEvent, _area: Rect) -> FormAction {
        use crossterm::event::Event;
        match handle_events(&mut self.state, true, &Event::Mouse(event)) {
            TextOutcome::TextChanged => {
                self.cache = self.state.value::<String>();
                self.dirty = true;
                FormAction::ValueChanged
            }
            _ => FormAction::Consumed,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> FormAction {
        // Ctrl+S=Submit, Ctrl+←=TabPrev, Ctrl+→=TabNext — handled before TextInputState
        // so our tab navigation is not consumed as "move cursor word left/right".
        if let Some(nav) = handle_form_nav(key) { return nav; }

        // Meta keys that TextInputState must not consume.
        match key.code {
            KeyCode::Tab     => return FormAction::FocusNext,
            KeyCode::BackTab => return FormAction::FocusPrev,
            KeyCode::Esc     => return FormAction::Cancel,
            KeyCode::Enter   => return FormAction::FocusNext,
            _ => {}
        }

        // max_len guard: refuse character insertion when at capacity.
        if self.max_len > 0 {
            if let KeyCode::Char(_) = key.code {
                if self.cache.chars().count() >= self.max_len { return FormAction::Consumed; }
            }
        }

        // Delegate to rat-widget TextInputState.
        match handle_events(&mut self.state, true, &Event::Key(key)) {
            TextOutcome::TextChanged => {
                self.cache = self.state.value::<String>();
                self.dirty = true;
                FormAction::ValueChanged
            }
            TextOutcome::Unchanged | TextOutcome::Changed => FormAction::Consumed,
            TextOutcome::Continue                         => FormAction::Unhandled,
        }
    }
}
