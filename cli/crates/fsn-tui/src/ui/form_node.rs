// Component-based form field architecture — the HTML element analogy for fsn-tui.
//
// Design principle: analogous to the HTML input element hierarchy.
//   HTMLElement → HTMLInputElement (text, password, email, …)
//   FormNode    → TextInputNode / SelectInputNode / …
//
// Each FormNode is a fully self-contained UI component:
//   • Owns its own state (value, cursor position, options, dirty flag)
//   • Renders itself: label + input box + hint (render)
//   • Renders overlays that must appear on top of siblings (render_overlay)
//   • Handles keyboard input, returns a typed FormAction
//
// This eliminates all per-field-type checks from events.rs (no more
// `is_select_field()`, `is_typing()`, etc.) — correct behavior is built in.
//
// Mouse handling is delegated to rat-widget (HandleEvent trait).
//
// Future extensions (same FormNode interface, different output backend):
//   fn render_html(&self, lang: Lang) -> String
//   fn to_json(&self, lang: Lang) -> serde_json::Value

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use crate::app::Lang;
use crate::ui::render_ctx::RenderCtx;

// ── Common navigation helper ──────────────────────────────────────────────────

/// Handle form-level navigation shortcuts — call at the top of every `FormNode::handle_key`.
///
/// Returns `Some(action)` for:
///   Ctrl+S           → Submit (works on all terminals; Ctrl+Enter does NOT)
///   Ctrl+←           → TabPrev
///   Ctrl+→           → TabNext
///
/// Tab / BackTab / Esc are intentionally excluded because widgets handle them
/// differently (e.g. TextArea: Tab=FocusNext, EnvTable: Tab=column-nav).
pub fn handle_form_nav(key: KeyEvent) -> Option<FormAction> {
    match key.code {
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(FormAction::Submit),
        KeyCode::Left      if key.modifiers.contains(KeyModifiers::CONTROL) => Some(FormAction::TabPrev),
        KeyCode::Right     if key.modifiers.contains(KeyModifiers::CONTROL) => Some(FormAction::TabNext),
        _ => None,
    }
}

// ── FormAction ────────────────────────────────────────────────────────────────

/// What a form node returns after handling a keyboard event.
/// The outer handler (events.rs) reacts to these without knowing field details.
#[derive(Debug, Clone, PartialEq)]
pub enum FormAction {
    /// Event consumed internally; no outer action needed.
    Consumed,
    /// Value was modified (triggers the form's `on_change` hook).
    ValueChanged,
    /// Move focus to the next node in the current tab.
    FocusNext,
    /// Move focus to the previous node in the current tab.
    FocusPrev,
    /// Advance to the next form tab (Ctrl+Right).
    TabNext,
    /// Go back to the previous form tab (Ctrl+Left).
    TabPrev,
    /// Enter was pressed: attempt to advance or submit the form.
    Submit,
    /// Close the form / pop the current screen (Esc).
    Cancel,
    /// Toggle the UI language (L/l key outside text input).
    LangToggle,
    /// Quit the application (Ctrl+C — handled before node dispatch).
    Quit,
    /// Event not handled by this node; fall through to the outer handler.
    Unhandled,
}

// ── FormNode trait ────────────────────────────────────────────────────────────

/// A UI component analogous to an HTML input element.
///
/// Each FormNode is fully self-contained:
/// - **State**: owns its value, cursor position, dirty flag
/// - **Render**: draws label + input box + hint; overlays (dropdowns) via `render_overlay`
/// - **Events**: handles keyboard input and returns a [`FormAction`] — no external dispatch
///
/// Mouse handling will be provided by rat-widget's `HandleEvent` trait.
/// Adding a new field type = implement `FormNode`. No changes needed in `events.rs`.
///
/// Implementing types: [`super::nodes::TextInputNode`], [`super::nodes::SelectInputNode`].
pub trait FormNode: std::fmt::Debug {
    // ── Identity ───────────────────────────────────────────────────────────

    /// Unique field identifier, used by `on_change` hooks to find siblings.
    fn key(&self) -> &'static str;
    /// i18n key for the label shown above the input.
    fn label_key(&self) -> &'static str;
    /// Optional i18n key for the hint line below the input.
    fn hint_key(&self) -> Option<&'static str>;
    /// Which tab this field belongs to (0-based).
    fn tab(&self) -> usize;
    /// Whether the field must be non-empty to submit.
    fn required(&self) -> bool;

    // ── Value ──────────────────────────────────────────────────────────────

    /// Raw value as typed by the user.
    fn value(&self) -> &str;
    /// Value for submit: returns the built-in default when the user left the field empty.
    fn effective_value(&self) -> &str;
    /// Set value programmatically (smart-defaults from `on_change`).
    fn set_value(&mut self, v: &str);
    /// Whether the user has manually edited this field.
    fn is_dirty(&self) -> bool;
    fn set_dirty(&mut self, v: bool);

    // ── Rendering ──────────────────────────────────────────────────────────

    /// How many rows this field needs in the form layout.
    ///
    /// Default: 4 (box-with-title 3 rows + hint 1 row).
    /// TextAreaNode overrides this based on its configured `visible_lines`.
    fn preferred_height(&self) -> u16 { 4 }

    /// Render the field (label-in-title + input box + hint) into `area`.
    fn render(&mut self, f: &mut RenderCtx<'_>, area: Rect, focused: bool, lang: Lang);

    /// Render a floating overlay (e.g., dropdown list) below the input box.
    /// Called *after* all fields are rendered so the overlay appears on top.
    /// Default: no-op (text inputs have no overlay).
    fn render_overlay(&mut self, _f: &mut RenderCtx<'_>, _available: Rect, _lang: Lang) {}

    // ── Input ──────────────────────────────────────────────────────────────

    /// Handle a keyboard event. Returns the action for the outer handler.
    fn handle_key(&mut self, key: KeyEvent) -> FormAction;

    // ── Validation ─────────────────────────────────────────────────────────

    fn is_filled(&self) -> bool {
        !self.effective_value().trim().is_empty()
    }
    fn is_valid(&self) -> bool {
        !self.required() || self.is_filled()
    }
}
