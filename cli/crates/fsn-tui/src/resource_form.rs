// ResourceKind enum, ResourceForm component, tab constants, and slugify helper.
//
// ResourceForm is a component-based generic form that holds a list of FormNode
// objects. Each node is fully self-contained: it renders itself, handles its
// own input, and knows how to hit-test mouse clicks.

use crate::ui::form_node::{FormAction, FormNode};

// ── Tab key constants ─────────────────────────────────────────────────────────

pub const PROJECT_TABS: &[&str] = &["form.tab.project", "form.tab.options"];
pub const SERVICE_TABS: &[&str] = &["form.tab.service", "form.tab.network", "form.tab.env"];
pub const HOST_TABS:    &[&str] = &["form.tab.host", "form.tab.system", "form.tab.dns"];
pub const BOT_TABS:     &[&str] = &["form.tab.bot", "form.tab.options"];

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
        Self { kind, tab_keys, active_tab: 0, active_field: 0, nodes, error: None, edit_id, on_change }
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
        let count = self.current_tab_indices().len();
        if count > 0 { self.active_field = (self.active_field + 1) % count; }
    }
    pub fn focus_prev(&mut self) {
        let count = self.current_tab_indices().len();
        if count > 0 {
            self.active_field = self.active_field.checked_sub(1).unwrap_or(count - 1);
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

        if action == FormAction::ValueChanged {
            if let Some(idx) = global_idx {
                let key = self.nodes[idx].key();
                (self.on_change)(&mut self.nodes, key);
            }
        }

        match action {
            FormAction::FocusNext   => { self.focus_next(); self.error = None; FormAction::Consumed }
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

    // ── Mouse click ────────────────────────────────────────────────────────

    /// Try to focus the node that was clicked. Returns its slot index on the tab.
    pub fn click_focus(&mut self, col: u16, row: u16) -> Option<usize> {
        let tab_indices = self.current_tab_indices();
        for (slot, &global_idx) in tab_indices.iter().enumerate() {
            if self.nodes[global_idx].hit_test(col, row) {
                self.active_field = slot;
                return Some(slot);
            }
        }
        None
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
