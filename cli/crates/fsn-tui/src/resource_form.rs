// ResourceKind enum, ResourceForm component, tab constants, and slugify helper.
//
// ResourceForm is a component-based generic form that holds a list of FormNode
// objects. Each node is fully self-contained: it renders itself, handles its
// own input, and knows how to hit-test mouse clicks.

use crate::ui::form_node::{FormAction, FormNode};

// ── Tab key constants ─────────────────────────────────────────────────────────

pub const PROJECT_TABS: &[&str] = &["form.tab.project"];
pub const SERVICE_TABS: &[&str] = &["form.tab.service"];
pub const HOST_TABS:    &[&str] = &["form.tab.host"];
pub const BOT_TABS:     &[&str] = &["form.tab.bot"];

// ── Resource kind ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind { Project, Service, Host, Bot }

impl ResourceKind {
    pub fn submit_key(self) -> &'static str {
        match self {
            ResourceKind::Project => "form.submit",
            ResourceKind::Service => "form.submit.service",
            ResourceKind::Host    => "form.submit.host",
            ResourceKind::Bot     => "form.submit.bot",
        }
    }

    /// i18n key for the form screen header — depends on whether this is a new or edit form.
    pub fn title_key(self, is_edit: bool) -> &'static str {
        match (self, is_edit) {
            (ResourceKind::Project, false) => "welcome.new_project",
            (ResourceKind::Project, true)  => "welcome.edit_project",
            (ResourceKind::Service, false) => "form.new_service",
            (ResourceKind::Service, true)  => "form.edit_service",
            (ResourceKind::Host,    false) => "form.new_host",
            (ResourceKind::Host,    true)  => "form.edit_host",
            (ResourceKind::Bot,     _)     => "form.tab.bot",
        }
    }
}

// ── ResourceForm ──────────────────────────────────────────────────────────────

/// Severity of a form-level error message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormErrorKind {
    /// Validation failure — user can fix it (shown in yellow).
    Validation,
    /// I/O or system failure — not user-actionable (shown in red).
    IoError,
}

pub struct ResourceForm {
    pub kind:         ResourceKind,
    /// i18n keys for tab headers.
    pub tab_keys:     &'static [&'static str],
    pub active_tab:   usize,
    /// Index within the CURRENT TAB's node list.
    pub active_field: usize,
    /// All field nodes across all tabs.
    pub nodes:        Vec<Box<dyn FormNode>>,
    pub error:        Option<String>,
    /// Severity of `error` — determines icon and color in `render_error`.
    pub error_kind:   FormErrorKind,
    /// Set to `true` after the user first navigates or changes a value.
    /// Enables live validation hints without showing warnings on a fresh form.
    pub touched:      bool,
    /// None = create, Some(id) = edit existing (slug for projects).
    pub edit_id:      Option<String>,
    /// Called after any value change: `(nodes, changed_field_key)`.
    pub on_change:    fn(&mut Vec<Box<dyn FormNode>>, &'static str),
}

impl std::fmt::Debug for ResourceForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceForm")
            .field("kind",         &self.kind)
            .field("active_tab",   &self.active_tab)
            .field("active_field", &self.active_field)
            .field("edit_id",      &self.edit_id)
            .finish()
    }
}

impl ResourceForm {
    pub fn new(
        kind:      ResourceKind,
        tab_keys:  &'static [&'static str],
        nodes:     Vec<Box<dyn FormNode>>,
        edit_id:   Option<String>,
        on_change: fn(&mut Vec<Box<dyn FormNode>>, &'static str),
    ) -> Self {
        Self { kind, tab_keys, active_tab: 0, active_field: 0, nodes, error: None, error_kind: FormErrorKind::Validation, touched: false, edit_id, on_change }
    }

    /// i18n key for the form screen header.
    pub fn title_key(&self) -> &'static str {
        self.kind.title_key(self.edit_id.is_some())
    }

    // ── Tab helpers ────────────────────────────────────────────────────────

    pub fn is_last_tab(&self) -> bool {
        self.active_tab == self.tab_keys.len().saturating_sub(1)
    }

    pub fn next_tab(&mut self) {
        self.active_tab   = (self.active_tab + 1) % self.tab_keys.len();
        self.active_field = 0;
    }
    pub fn prev_tab(&mut self) {
        self.active_tab   = self.active_tab.checked_sub(1).unwrap_or(self.tab_keys.len() - 1);
        self.active_field = 0;
    }

    // ── Node access ────────────────────────────────────────────────────────

    /// Indices into `self.nodes` for fields on the active tab.
    pub fn current_tab_indices(&self) -> Vec<usize> {
        self.nodes.iter().enumerate()
            .filter(|(_, n)| n.tab() == self.active_tab)
            .map(|(i, _)| i)
            .collect()
    }


    /// Global index of the focused node, or `None`.
    pub fn focused_node_global_idx(&self) -> Option<usize> {
        self.current_tab_indices().get(self.active_field).copied()
    }

    pub fn focused_node(&self) -> Option<&dyn FormNode> {
        self.focused_node_global_idx().map(|i| self.nodes[i].as_ref())
    }

    pub fn focused_node_mut(&mut self) -> Option<&mut dyn FormNode> {
        let idx = self.focused_node_global_idx()?;
        Some(self.nodes[idx].as_mut())
    }

    // ── Focus movement ─────────────────────────────────────────────────────

    pub fn focus_next(&mut self) {
        let indices = self.current_tab_indices();
        let count = indices.len();
        if count == 0 { return; }

        // Advance then skip any non-focusable nodes (e.g. SectionNode).
        let mut next = self.active_field + 1;
        while next < count && !self.nodes[indices[next]].is_focusable() {
            next += 1;
        }
        if next >= count {
            // Past the last field on this tab → advance to the next tab.
            self.next_tab();
            // Skip leading non-focusable nodes on the new tab.
            let new_indices = self.current_tab_indices();
            let mut start = 0;
            while start + 1 < new_indices.len() && !self.nodes[new_indices[start]].is_focusable() {
                start += 1;
            }
            self.active_field = start;
        } else {
            self.active_field = next;
        }
    }

    pub fn focus_prev(&mut self) {
        let indices = self.current_tab_indices();
        let count = indices.len();
        if count == 0 { return; }

        if self.active_field == 0
            || (0..self.active_field).all(|i| !self.nodes[indices[i]].is_focusable())
        {
            // No focusable field before this one on this tab → go to previous tab.
            self.prev_tab();
            let new_indices = self.current_tab_indices();
            // Find the last focusable node on the new tab.
            let last = new_indices.len().saturating_sub(1);
            let mut end = last;
            while end > 0 && !self.nodes[new_indices[end]].is_focusable() {
                end -= 1;
            }
            self.active_field = end;
        } else {
            let mut prev = self.active_field - 1;
            while prev > 0 && !self.nodes[indices[prev]].is_focusable() {
                prev -= 1;
            }
            self.active_field = prev;
        }
    }

    // ── Key dispatch ───────────────────────────────────────────────────────

    /// Dispatch a key event to the focused node, handle focus/tab navigation,
    /// and fire `on_change` when a value was modified.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> FormAction {
        let global_idx = self.focused_node_global_idx();
        let action = if let Some(idx) = global_idx {
            self.nodes[idx].handle_key(key)
        } else {
            FormAction::Unhandled
        };

        if action == FormAction::ValueChanged || action == FormAction::AcceptAndNext {
            if let Some(idx) = global_idx {
                let key = self.nodes[idx].key();
                (self.on_change)(&mut self.nodes, key);
            }
        }

        match action {
            FormAction::FocusNext | FormAction::AcceptAndNext => { self.focus_next(); self.error = None; FormAction::Consumed }
            FormAction::FocusPrev   => { self.focus_prev(); self.error = None; FormAction::Consumed }
            FormAction::TabNext     => { self.next_tab();   self.error = None; FormAction::Consumed }
            FormAction::TabPrev     => { self.prev_tab();   self.error = None; FormAction::Consumed }
            FormAction::ValueChanged => FormAction::Consumed,
            other => other,
        }
    }

    // ── Validation ─────────────────────────────────────────────────────────

    pub fn is_dirty(&self) -> bool {
        self.nodes.iter().any(|n| n.is_dirty())
    }

    pub fn tab_missing_count(&self, tab_idx: usize) -> usize {
        self.nodes.iter()
            .filter(|n| n.tab() == tab_idx && n.required() && !n.is_filled())
            .count()
    }

    pub fn missing_required(&self) -> Vec<&'static str> {
        self.nodes.iter()
            .filter(|n| n.required() && !n.is_filled())
            .map(|n| n.label_key())
            .collect()
    }

    // ── Value access ───────────────────────────────────────────────────────

    pub fn field_value(&self, key: &str) -> String {
        self.nodes.iter().find(|n| n.key() == key)
            .map(|n| n.effective_value().to_string())
            .unwrap_or_default()
    }

    pub fn set_field_value(&mut self, key: &str, value: &str) {
        if let Some(n) = self.nodes.iter_mut().find(|n| n.key() == key) {
            n.set_value(value);
        }
    }

}

// ── Slugify helper ────────────────────────────────────────────────────────────

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        match c {
            'a'..='z' | '0'..='9' | '.' => out.push(c),
            ' ' | '_' | '-' => { if !out.ends_with('-') { out.push('-'); } }
            _ => {}
        }
    }
    out.trim_matches('-').to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("My Project"),  "my-project");
        assert_eq!(slugify("hello world"), "hello-world");
    }

    #[test]
    fn slugify_collapses_separators() {
        assert_eq!(slugify("a  b"), "a-b");
        assert_eq!(slugify("a--b"), "a-b");
        assert_eq!(slugify("a_b"),  "a-b");
    }

    #[test]
    fn slugify_strips_leading_trailing_dashes() {
        assert_eq!(slugify("-foo-"), "foo");
        assert_eq!(slugify("_bar_"), "bar");
    }

    #[test]
    fn slugify_removes_special_chars() {
        assert_eq!(slugify("my!project@2024"), "myproject2024");
    }

    #[test]
    fn slugify_preserves_dots() {
        assert_eq!(slugify("v1.2.3"), "v1.2.3");
    }

    #[test]
    fn form_error_kind_defaults_to_validation() {
        use crate::resource_form::{ResourceKind, FormErrorKind};
        let form = ResourceForm::new(
            ResourceKind::Project,
            &["form.tab.project"],
            vec![],
            None,
            |_, _| {},
        );
        assert_eq!(form.error_kind, FormErrorKind::Validation);
        assert!(form.error.is_none());
    }
}
