// Application state and main event loop.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::sysinfo::SysInfo;
use crate::ui;

// ── Screens ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Dashboard,
    NewProject,
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

// ── Service table ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub name:         String,
    pub service_type: String,
    pub domain:       String,
    pub status:       ServiceStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Error,
    Unknown,
}

impl ServiceStatus {
    pub fn i18n_key(self) -> &'static str {
        match self {
            ServiceStatus::Running => "status.running",
            ServiceStatus::Stopped => "status.stopped",
            ServiceStatus::Error   => "status.error",
            ServiceStatus::Unknown => "status.unknown",
        }
    }
}

// ── New Project form ──────────────────────────────────────────────────────────

/// Which tab is active in the New Project form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormTab {
    Project = 0,
    Server  = 1,
    Options = 2,
}

impl FormTab {
    pub fn from_index(i: usize) -> Self {
        match i { 1 => FormTab::Server, 2 => FormTab::Options, _ => FormTab::Project }
    }
    pub fn count() -> usize { 3 }
    pub fn i18n_key(self) -> &'static str {
        match self {
            FormTab::Project => "form.tab.project",
            FormTab::Server  => "form.tab.server",
            FormTab::Options => "form.tab.options",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormFieldType {
    Text,
    Email,
    Ip,
    Secret,
    Path,
    Select,  // uses `options` list
}

#[derive(Debug, Clone)]
pub struct FormField {
    /// Config key (written to project.toml / host.toml)
    pub key:        &'static str,
    /// i18n key for the label
    pub label_key:  &'static str,
    /// i18n key for the description / hint below the field
    pub hint_key:   Option<&'static str>,
    pub tab:        FormTab,
    pub required:   bool,
    pub field_type: FormFieldType,
    /// Current text value (or selected option index as string for Select)
    pub value:      String,
    /// Cursor position within `value`
    pub cursor:     usize,
    /// For Select fields — available options (i18n keys or static strings)
    pub options:    Vec<&'static str>,
}

impl FormField {
    fn new(key: &'static str, label_key: &'static str, tab: FormTab, required: bool, field_type: FormFieldType) -> Self {
        Self { key, label_key, hint_key: None, tab, required, field_type,
               value: String::new(), cursor: 0, options: vec![] }
    }
    fn hint(mut self, k: &'static str) -> Self { self.hint_key = Some(k); self }
    fn default_val(mut self, v: &str) -> Self { self.value = v.to_string(); self.cursor = v.len(); self }
    fn opts(mut self, o: Vec<&'static str>) -> Self { self.options = o; self }
}

#[derive(Debug)]
pub struct NewProjectForm {
    pub active_tab:   usize,        // 0..FormTab::count()-1
    pub active_field: usize,        // index into `fields` filtered by active_tab
    pub fields:       Vec<FormField>,
    pub error:        Option<String>,
}

impl NewProjectForm {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".into());
        let fields = vec![
            // ── Tab: Project ────────────────────────────────────────────────
            FormField::new("name",   "form.project.name",   FormTab::Project, true,  FormFieldType::Text)
                .hint("form.project.name.hint"),
            FormField::new("domain", "form.project.domain", FormTab::Project, true,  FormFieldType::Text)
                .hint("form.project.domain.hint"),
            FormField::new("path",   "form.project.path",   FormTab::Project, true,  FormFieldType::Path)
                .default_val(&format!("{}/fsn", home))
                .hint("form.project.path.hint"),
            FormField::new("contact_email", "form.project.email", FormTab::Project, true, FormFieldType::Email)
                .hint("form.project.email.hint"),

            // ── Tab: Server ─────────────────────────────────────────────────
            FormField::new("host_ip",       "form.server.ip",          FormTab::Server, true,  FormFieldType::Ip)
                .hint("form.server.ip.hint"),
            FormField::new("dns_provider",  "form.server.dns_provider",FormTab::Server, true,  FormFieldType::Select)
                .opts(vec!["Hetzner DNS", "Cloudflare", "Manual"])
                .hint("form.server.dns_provider.hint"),
            FormField::new("dns_api_token", "form.server.dns_token",   FormTab::Server, true,  FormFieldType::Secret)
                .hint("form.server.dns_token.hint"),

            // ── Tab: Options ─────────────────────────────────────────────────
            FormField::new("description", "form.options.description", FormTab::Options, false, FormFieldType::Text),
            FormField::new("language",    "form.options.language",    FormTab::Options, false, FormFieldType::Select)
                .opts(vec!["de", "en", "fr", "es", "it", "pt"])
                .default_val("de"),
            FormField::new("version",     "form.options.version",     FormTab::Options, false, FormFieldType::Text)
                .default_val("0.1.0"),
        ];

        Self { active_tab: 0, active_field: 0, fields, error: None }
    }

    /// Indices of fields belonging to the active tab.
    pub fn tab_field_indices(&self) -> Vec<usize> {
        let tab = FormTab::from_index(self.active_tab);
        self.fields.iter().enumerate()
            .filter(|(_, f)| f.tab == tab)
            .map(|(i, _)| i)
            .collect()
    }

    /// The currently focused field (global index).
    pub fn focused_field_idx(&self) -> Option<usize> {
        let indices = self.tab_field_indices();
        indices.get(self.active_field).copied()
    }

    /// Move focus to next field in tab; returns true if wrapped (stay in tab).
    pub fn focus_next(&mut self) {
        let count = self.tab_field_indices().len();
        if count == 0 { return; }
        self.active_field = (self.active_field + 1) % count;
    }

    pub fn focus_prev(&mut self) {
        let count = self.tab_field_indices().len();
        if count == 0 { return; }
        self.active_field = self.active_field.checked_sub(1).unwrap_or(count - 1);
    }

    pub fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % FormTab::count();
        self.active_field = 0;
    }
    pub fn prev_tab(&mut self) {
        self.active_tab = self.active_tab.checked_sub(1).unwrap_or(FormTab::count() - 1);
        self.active_field = 0;
    }

    /// Insert char at cursor of focused field.
    pub fn insert_char(&mut self, c: char) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            f.value.insert(f.cursor, c);
            f.cursor += c.len_utf8();
        }
    }

    /// Delete char before cursor.
    pub fn backspace(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if f.cursor > 0 {
                let prev = f.value[..f.cursor]
                    .char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
                f.value.remove(prev);
                f.cursor = prev;
            }
        }
    }

    pub fn cursor_left(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if f.cursor > 0 {
                f.cursor = f.value[..f.cursor]
                    .char_indices().next_back().map(|(i, _)| i).unwrap_or(0);
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

    /// Delete char at cursor position (forward delete).
    pub fn delete_char(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            if f.cursor < f.value.len() {
                let next = f.value[f.cursor..].chars().next()
                    .map(|c| f.cursor + c.len_utf8())
                    .unwrap_or(f.cursor);
                f.value.drain(f.cursor..next);
            }
        }
    }

    pub fn cursor_home(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            self.fields[idx].cursor = 0;
        }
    }

    pub fn cursor_end(&mut self) {
        if let Some(idx) = self.focused_field_idx() {
            let f = &mut self.fields[idx];
            f.cursor = f.value.len();
        }
    }

    /// Cycle a Select field's value forward.
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

    /// Cycle a Select field's value backward.
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

    /// Validate — returns list of missing required fields.
    pub fn missing_required(&self) -> Vec<&'static str> {
        self.fields.iter()
            .filter(|f| f.required && f.value.trim().is_empty())
            .map(|f| f.label_key)
            .collect()
    }
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
    pub new_project:        Option<NewProjectForm>,
    last_refresh:           Instant,
}

#[derive(Debug, Clone)]
pub struct LogsState {
    pub service_name: String,
    pub lines:        Vec<String>,
    pub scroll:       usize,
}

impl AppState {
    pub fn new(sysinfo: SysInfo, services: Vec<ServiceRow>) -> Self {
        let screen = if services.is_empty() { Screen::Welcome } else { Screen::Dashboard };
        Self {
            screen, lang: Lang::De, sysinfo, services,
            selected: 0, logs_overlay: None, lang_dropdown_open: false,
            should_quit: false, welcome_focus: 0, new_project: None,
            last_refresh: Instant::now(),
        }
    }

    pub fn t<'a>(&self, key: &'a str) -> &'a str {
        crate::i18n::t(self.lang, key)
    }
}

// ── Main loop ─────────────────────────────────────────────────────────────────

pub fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    state:    &mut AppState,
    root:     &Path,
) -> Result<()> {
    const POLL_MS:      u64 = 250;
    const REFRESH_SECS: u64 = 5;

    loop {
        terminal.draw(|f| ui::render(f, state))?;

        if event::poll(Duration::from_millis(POLL_MS))? {
            if let Event::Key(key) = event::read()? {
                crate::events::handle(key, state, root)?;
            }
        }

        if state.should_quit { break; }

        if state.last_refresh.elapsed() >= Duration::from_secs(REFRESH_SECS) {
            state.sysinfo = SysInfo::collect();
            state.last_refresh = Instant::now();
        }
    }

    Ok(())
}
