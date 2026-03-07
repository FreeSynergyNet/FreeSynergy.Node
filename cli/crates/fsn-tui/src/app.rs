// Application state and main event loop.

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use fsn_core::config::project::ProjectConfig;
pub use fsn_core::state::actual::RunState;

use crate::sysinfo::SysInfo;
use crate::ui;

// ── Screens ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Dashboard,
    NewProject,
}

// ── Dashboard focus ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashFocus {
    Sidebar,
    Services,
}

// ── Language ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
}

impl Lang {
    pub fn toggle(self) -> Self {
        match self { Lang::De => Lang::En, Lang::En => Lang::De }
    }
    pub fn label(self) -> &'static str {
        match self { Lang::De => "DE", Lang::En => "EN" }
    }
}

// ── Project handle (loaded from disk) ─────────────────────────────────────────

/// A project loaded from `projects/{slug}/{slug}.project.toml`.
/// `config` is the parsed `ProjectConfig` from fsn-core.
/// `slug` and `toml_path` are TUI-level metadata not stored inside the TOML.
#[derive(Debug, Clone)]
pub struct ProjectHandle {
    pub slug:      String,
    pub toml_path: std::path::PathBuf,
    pub config:    ProjectConfig,
}

impl ProjectHandle {
    /// Convenience: project display name.
    pub fn name(&self) -> &str { &self.config.project.name }
    /// Convenience: primary domain.
    pub fn domain(&self) -> &str { &self.config.project.domain }
    /// Convenience: contact e-mail (first non-empty of email / acme_email).
    pub fn email(&self) -> &str {
        self.config.project.contact.as_ref()
            .and_then(|c| c.email.as_deref().or(c.acme_email.as_deref()))
            .unwrap_or("")
    }
    /// Convenience: install directory.
    pub fn install_dir(&self) -> &str {
        self.config.project.install_dir.as_deref().unwrap_or("")
    }
}

// ── Service table ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub name:         String,
    pub service_type: String,
    pub domain:       String,
    pub status:       RunState,
}

/// Map `RunState` to an i18n key for the status column.
pub fn run_state_i18n(state: RunState) -> &'static str {
    match state {
        RunState::Running => "status.running",
        RunState::Stopped => "status.stopped",
        RunState::Failed  => "status.error",
        RunState::Missing => "status.unknown",
    }
}

// ── Resource form — generic editor for any Resource type ──────────────────────

/// Tab key constants for project forms.
pub const PROJECT_TABS: &[&str] = &["form.tab.project", "form.tab.options"];
/// Tab key constants for service forms.
pub const SERVICE_TABS: &[&str] = &["form.tab.service", "form.tab.options"];

/// Which resource type the form is editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind { Project, Service }

impl ResourceKind {
    pub fn new_title_key(self) -> &'static str {
        match self {
            ResourceKind::Project => "welcome.new_project",
            ResourceKind::Service => "form.new_service",
        }
    }
    pub fn edit_title_key(self) -> &'static str {
        match self {
            ResourceKind::Project => "welcome.edit_project",
            ResourceKind::Service => "form.edit_service",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormFieldType { Text, Email, Ip, Secret, Path, Select }

#[derive(Debug, Clone)]
pub struct FormField {
    pub key:        &'static str,
    pub label_key:  &'static str,
    pub hint_key:   Option<&'static str>,
    /// Index into `ResourceForm.tab_keys` — which tab this field belongs to.
    pub tab:        usize,
    pub required:   bool,
    pub field_type: FormFieldType,
    pub value:      String,
    pub cursor:     usize,
    pub dirty:      bool,
    pub options:    Vec<&'static str>,
    /// Optional display mapper for Select fields (code → human label).
    pub display_fn: Option<fn(&str) -> &'static str>,
}

impl FormField {
    pub fn new(key: &'static str, label_key: &'static str, tab: usize, required: bool, field_type: FormFieldType) -> Self {
        Self { key, label_key, hint_key: None, tab, required, field_type,
               value: String::new(), cursor: 0, dirty: false, options: vec![], display_fn: None }
    }
    pub fn hint(mut self, k: &'static str) -> Self { self.hint_key = Some(k); self }
    pub fn default_val(mut self, v: &str) -> Self { self.value = v.to_string(); self.cursor = v.len(); self }
    pub fn opts(mut self, o: Vec<&'static str>) -> Self { self.options = o; self }
    pub fn dirty(mut self) -> Self { self.dirty = true; self }
    pub fn display(mut self, f: fn(&str) -> &'static str) -> Self { self.display_fn = Some(f); self }

    /// Human-readable display value (for Select fields with a display_fn).
    pub fn display_value(&self) -> &str {
        if let Some(f) = self.display_fn {
            let s = f(&self.value);
            if s.is_empty() { &self.value } else { s }
        } else {
            &self.value
        }
    }
}

/// Generic resource editor form.
/// Works for projects, services, and any future resource type.
#[derive(Debug)]
pub struct ResourceForm {
    pub kind:         ResourceKind,
    /// i18n keys for each tab header.
    pub tab_keys:     &'static [&'static str],
    pub active_tab:   usize,
    pub active_field: usize,
    pub fields:       Vec<FormField>,
    pub error:        Option<String>,
    /// None = create new, Some(id) = edit existing (for projects: slug).
    pub edit_id:      Option<String>,
    /// Hook called after any field edit — resource-specific smart defaults.
    pub on_change:    fn(&mut ResourceForm, usize),
}

impl ResourceForm {
    pub fn new(
        kind:      ResourceKind,
        tab_keys:  &'static [&'static str],
        fields:    Vec<FormField>,
        edit_id:   Option<String>,
        on_change: fn(&mut ResourceForm, usize),
    ) -> Self {
        Self { kind, tab_keys, active_tab: 0, active_field: 0, fields, error: None, edit_id, on_change }
    }

    pub fn is_last_tab(&self) -> bool {
        self.active_tab == self.tab_keys.len().saturating_sub(1)
    }

    pub fn tab_field_indices(&self) -> Vec<usize> {
        let tab = self.active_tab;
        self.fields.iter().enumerate()
            .filter(|(_, f)| f.tab == tab)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn focused_field_idx(&self) -> Option<usize> {
        self.tab_field_indices().get(self.active_field).copied()
    }

    pub fn focus_next(&mut self) {
        let count = self.tab_field_indices().len();
        if count > 0 { self.active_field = (self.active_field + 1) % count; }
    }
    pub fn focus_prev(&mut self) {
        let count = self.tab_field_indices().len();
        if count > 0 { self.active_field = self.active_field.checked_sub(1).unwrap_or(count - 1); }
    }
    pub fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % self.tab_keys.len();
        self.active_field = 0;
    }
    pub fn prev_tab(&mut self) {
        self.active_tab = self.active_tab.checked_sub(1).unwrap_or(self.tab_keys.len() - 1);
        self.active_field = 0;
    }

    pub fn insert_char(&mut self, c: char) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            f.value.insert(f.cursor, c);
            f.cursor += c.len_utf8();
            f.dirty = true;
            let hook = self.on_change;
            hook(self, idx);
        }
    }

    pub fn backspace(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let changed = {
                let f = &mut self.fields[idx];
                if f.cursor > 0 {
                    let prev = f.value[..f.cursor].char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
                    f.value.remove(prev);
                    f.cursor = prev;
                    f.dirty = true;
                    true
                } else { false }
            };
            if changed { let hook = self.on_change; hook(self, idx); }
        }
    }

    pub fn cursor_left(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if f.cursor > 0 {
                f.cursor = f.value[..f.cursor].char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
            }
        }
    }
    pub fn cursor_right(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if f.cursor < f.value.len() {
                let next = f.value[f.cursor..].chars().next().map(|c| f.cursor + c.len_utf8()).unwrap_or(f.cursor);
                f.cursor = next;
            }
        }
    }
    pub fn delete_char(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let changed = {
                let f = &mut self.fields[idx];
                if f.cursor < f.value.len() {
                    let next = f.value[f.cursor..].chars().next().map(|c| f.cursor + c.len_utf8()).unwrap_or(f.cursor);
                    f.value.drain(f.cursor..next);
                    f.dirty = true;
                    true
                } else { false }
            };
            if changed { let hook = self.on_change; hook(self, idx); }
        }
    }
    pub fn cursor_home(&mut self) {
        if let Some(idx) = self.focused_field_idx() { self.fields[idx].cursor = 0; }
    }
    pub fn cursor_end(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            f.cursor = f.value.len();
        }
    }

    pub fn select_next(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if matches!(f.field_type, FormFieldType::Select) && !f.options.is_empty() {
                let cur = f.options.iter().position(|&o| o == f.value).unwrap_or(0);
                let next = (cur + 1) % f.options.len();
                f.value = f.options[next].to_string();
            }
        }
    }
    pub fn select_prev(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if matches!(f.field_type, FormFieldType::Select) && !f.options.is_empty() {
                let cur = f.options.iter().position(|&o| o == f.value).unwrap_or(0);
                let prev = if cur == 0 { f.options.len() - 1 } else { cur - 1 };
                f.value = f.options[prev].to_string();
            }
        }
    }

    pub fn tab_missing_count(&self, tab_idx: usize) -> usize {
        self.fields.iter()
            .filter(|f| f.tab == tab_idx && f.required && f.value.trim().is_empty())
            .count()
    }

    pub fn field_value(&self, key: &str) -> String {
        self.fields.iter().find(|f| f.key == key).map(|f| f.value.clone()).unwrap_or_default()
    }

    pub fn set_select_by_index(&mut self, option_idx: usize) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if matches!(f.field_type, FormFieldType::Select) && option_idx < f.options.len() {
                f.value = f.options[option_idx].to_string();
            }
        }
    }

    pub fn missing_required(&self) -> Vec<&'static str> {
        self.fields.iter()
            .filter(|f| f.required && f.value.trim().is_empty())
            .map(|f| f.label_key)
            .collect()
    }
}

/// Backwards-compat alias so call sites still compile during transition.
pub type NewProjectForm = ResourceForm;

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

// ── Full application state ────────────────────────────────────────────────────

pub struct AppState {
    pub screen:             Screen,
    pub lang:               Lang,
    pub sysinfo:            SysInfo,
    pub services:           Vec<ServiceRow>,
    pub selected:           usize,
    pub logs_overlay:       Option<LogsState>,
    pub lang_dropdown_open: bool,
    pub should_quit:        bool,
    /// Focused button on welcome screen (0=New, 1=Open)
    pub welcome_focus:      usize,
    pub current_form:       Option<ResourceForm>,
    /// True when last keypress included CONTROL — switches hint bar to Ctrl shortcuts.
    pub ctrl_hint:          bool,
    /// Loaded projects from disk.
    pub projects:           Vec<ProjectHandle>,
    pub selected_project:   usize,
    pub dash_focus:         DashFocus,
    /// True = waiting for delete-confirm (J/N).
    pub dash_confirm:       bool,
    last_refresh:           Instant,
    /// Last known Podman container statuses (name → RunState).
    last_podman_statuses:   HashMap<String, RunState>,
}

#[derive(Debug, Clone)]
pub struct LogsState {
    pub service_name: String,
    pub lines:        Vec<String>,
    pub scroll:       usize,
}

impl AppState {
    pub fn new(sysinfo: SysInfo, projects: Vec<ProjectHandle>) -> Self {
        Self {
            screen: Screen::Welcome, lang: Lang::De, sysinfo, services: vec![],
            selected: 0, logs_overlay: None, lang_dropdown_open: false,
            should_quit: false, welcome_focus: 0, current_form: None,
            ctrl_hint: false, projects, selected_project: 0,
            dash_focus: DashFocus::Sidebar, dash_confirm: false,
            last_refresh: Instant::now(),
            last_podman_statuses: HashMap::new(),
        }
    }

    /// Apply freshly-queried Podman statuses and rebuild the service list.
    pub fn apply_podman_status(&mut self, statuses: HashMap<String, RunState>) {
        self.last_podman_statuses = statuses;
        self.rebuild_services();
    }

    /// Rebuild `self.services` from the current project's desired state
    /// merged with the last known Podman container statuses.
    pub fn rebuild_services(&mut self) {
        let Some(proj) = self.projects.get(self.selected_project) else {
            self.services.clear();
            return;
        };
        let domain = proj.domain().to_string();
        self.services = proj.config.load.services.iter()
            .map(|(name, entry)| ServiceRow {
                status:       self.last_podman_statuses.get(name).copied().unwrap_or(RunState::Missing),
                domain:       format!("{}.{}", name, domain),
                service_type: entry.service_class.clone(),
                name:         name.clone(),
            })
            .collect();
        // Clamp selection
        if self.selected >= self.services.len() && !self.services.is_empty() {
            self.selected = self.services.len() - 1;
        }
    }

    pub fn t<'a>(&self, key: &'a str) -> &'a str {
        crate::i18n::t(self.lang, key)
    }
}

// ── Main loop ─────────────────────────────────────────────────────────────────

pub fn run_loop(
    terminal:     &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    state:        &mut AppState,
    root:         &Path,
    reconcile_rx: mpsc::Receiver<HashMap<String, RunState>>,
) -> Result<()> {
    const POLL_MS:      u64 = 250;
    const REFRESH_SECS: u64 = 5;

    loop {
        terminal.draw(|f| ui::render(f, state))?;

        if event::poll(Duration::from_millis(POLL_MS))? {
            match event::read()? {
                Event::Key(key) => crate::events::handle(key, state, root)?,
                Event::Mouse(mouse) => crate::events::handle_mouse(mouse, state)?,
                _ => {}
            }
        }

        if state.should_quit { break; }

        // Apply latest Podman status updates from background reconciler
        while let Ok(statuses) = reconcile_rx.try_recv() {
            state.apply_podman_status(statuses);
        }

        if state.last_refresh.elapsed() >= Duration::from_secs(REFRESH_SECS) {
            state.sysinfo = SysInfo::collect();
            state.last_refresh = Instant::now();
        }
    }

    Ok(())
}
