// SectionNode — visual section separator inside a form.
//
// Design Pattern: Null Object — implements FormNode fully but carries no
// interactive state. `is_focusable()` returns false, so focus navigation
// skips over it automatically. `handle_key()` always returns Unhandled.
//
// Renders as:
//   ── Section Label ──────────────────────────────────
// (DarkGray horizontal rule with a label on the left.)

use crossterm::event::KeyEvent;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use rat_widget::paragraph::{Paragraph, ParagraphState};

use crate::app::Lang;
use crate::ui::form_node::{FormAction, FormNode};
use crate::ui::render_ctx::RenderCtx;

#[derive(Debug)]
pub struct SectionNode {
    pub key:       &'static str,
    pub label_key: &'static str,
    pub tab:       usize,
}

impl SectionNode {
    pub fn new(key: &'static str, label_key: &'static str, tab: usize) -> Self {
        Self { key, label_key, tab }
    }
}

impl FormNode for SectionNode {
    fn key(&self)       -> &'static str         { self.key }
    fn label_key(&self) -> &'static str         { self.label_key }
    fn hint_key(&self)  -> Option<&'static str> { None }
    fn tab(&self)       -> usize                { self.tab }
    fn required(&self)  -> bool                 { false }

    fn value(&self)           -> &str { "" }
    fn effective_value(&self) -> &str { "" }
    fn set_value(&mut self, _: &str) {}
    fn is_dirty(&self)  -> bool      { false }
    fn set_dirty(&mut self, _: bool) {}

    // Not interactive — focus never lands here.
    fn is_focusable(&self) -> bool { false }

    // 2 rows: 1 for the rule + label, 1 for spacing below.
    fn preferred_height(&self) -> u16 { 2 }

    fn render(&mut self, f: &mut RenderCtx<'_>, area: Rect, _focused: bool, lang: Lang) {
        if area.height == 0 { return; }
        let label = crate::i18n::t(lang, self.label_key);
        // Build "── Label ──────────────..." filling the full width.
        let prefix   = "── ";
        let suffix   = " ";
        let rule_len = (area.width as usize)
            .saturating_sub(prefix.len() + label.chars().count() + suffix.len());
        let rule: String = "─".repeat(rule_len);
        let line = Line::from(vec![
            Span::styled(prefix,  Style::default().fg(Color::DarkGray)),
            Span::styled(label,   Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(suffix,  Style::default().fg(Color::DarkGray)),
            Span::styled(rule,    Style::default().fg(Color::DarkGray)),
        ]);
        let rule_area = Rect { height: 1, ..area };
        f.render_stateful_widget(
            Paragraph::new(line),
            rule_area,
            &mut ParagraphState::new(),
        );
    }

    fn handle_key(&mut self, _key: KeyEvent) -> FormAction {
        FormAction::Unhandled
    }
}
