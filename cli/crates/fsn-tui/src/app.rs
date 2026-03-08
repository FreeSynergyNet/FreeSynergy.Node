// Application state and main event loop.

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use fsn_core::config::project::{ProjectConfig, ServiceInstanceConfig};
use fsn_core::config::host::HostConfig;
use fsn_core::error::FsnError;
use fsn_core::resource::{HostResource, ProjectResource, Resource, ServiceResource};
pub use fsn_core::state::actual::RunState;

use crate::sysinfo::SysInfo;
use crate::ui;
use crate::ui::form_node::{FormAction, FormNode};

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

// ── Sidebar item ──────────────────────────────────────────────────────────────

/// The action triggered when a sidebar item is activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarAction { NewProject, NewHost }

/// One navigable row in the sidebar.
///
/// Analogous to a DOM element — each variant knows its own visual appearance
/// and the action it triggers when selected. Generic navigation code uses
/// `is_selectable()` without pattern-matching on the variant.
#[derive(Debug, Clone)]
pub enum SidebarItem {
    /// Non-navigable section header (i18n key).
    Section(&'static str),
    /// A project entry — selecting updates `selected_project`.
    Project { slug: String, name: String },
    /// A host entry — selecting updates `selected_host`.
    Host    { slug: String, name: String },
    /// An action button ("+ New Project", "+ New Host").
    Action  { label_key: &'static str, kind: SidebarAction },
}

impl SidebarItem {
    /// Returns `true` for all variants except `Section` (headers are not navigable).
    pub fn is_selectable(&self) -> bool {
        !matches!(self, SidebarItem::Section(_))
    }
    /// If this item is an `Action`, returns its kind.
    pub fn action_kind(&self) -> Option<SidebarAction> {
        if let SidebarItem::Action { kind, .. } = self { Some(*kind) } else { None }
    }
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

// ── Project handle ────────────────────────────────────────────────────────────

/// Runtime wrapper for a loaded project config.
///
/// Combines the on-disk `ProjectConfig` with the filesystem slug (derived from
/// the filename stem) and the absolute path to the TOML file.
///
/// Implements [`Resource`] and [`ProjectResource`] — use `&dyn ProjectResource`
/// when project-level properties are needed in generic code.
#[derive(Debug, Clone)]
pub struct ProjectHandle {
    /// Filesystem slug — derived from `{name}.project.toml` filename stem.
    pub slug:      String,
    /// Absolute path to the `.project.toml` file.
    pub toml_path: std::path::PathBuf,
    /// Parsed project configuration.
    pub config:    ProjectConfig,
}

impl ProjectHandle {
    pub fn name(&self)        -> &str { &self.config.project.name }
    pub fn domain(&self)      -> &str { &self.config.project.domain }
    pub fn install_dir(&self) -> &str {
        self.config.project.install_dir.as_deref().unwrap_or("")
    }
    pub fn email(&self) -> &str {
        self.config.project.contact.as_ref()
            .and_then(|c| c.email.as_deref().or(c.acme_email.as_deref()))
            .unwrap_or("")
    }
}

impl Resource for ProjectHandle {
    fn kind(&self) -> &'static str { "project" }
    fn id(&self)   -> &str         { &self.slug }
    fn display_name(&self) -> &str { &self.config.project.name }
    fn description(&self)  -> Option<&str> { self.config.project.description.as_deref() }
    fn validate(&self) -> Result<(), FsnError> { self.config.validate() }
}

impl ProjectResource for ProjectHandle {
    fn domain(&self)        -> &str           { &self.config.project.domain }
    fn contact_email(&self) -> Option<&str>   { self.config.contact_email() }
    fn languages(&self)     -> &[String]      { &self.config.project.languages }
    fn install_dir(&self)   -> Option<&str>   { self.config.project.install_dir.as_deref() }
}

// ── Host handle ───────────────────────────────────────────────────────────────

/// Runtime wrapper for a loaded host config.
///
/// Combines the on-disk `HostConfig` with the filesystem slug and absolute path.
///
/// Implements [`Resource`] and [`HostResource`].
#[derive(Debug, Clone)]
pub struct HostHandle {
    /// Filesystem slug — derived from `{name}.host.toml` filename stem.
    pub slug:      String,
    /// Absolute path to the `.host.toml` file.
    pub toml_path: std::path::PathBuf,
    /// Parsed host configuration.
    pub config:    HostConfig,
}

impl HostHandle {
    pub fn name(&self) -> &str { &self.config.host.name }
    pub fn addr(&self) -> &str { self.config.host.addr() }
}

impl Resource for HostHandle {
    fn kind(&self) -> &'static str { "host" }
    fn id(&self)   -> &str         { &self.slug }
    fn display_name(&self) -> &str {
        self.config.host.alias.as_deref().unwrap_or(&self.config.host.name)
    }
    fn tags(&self)  -> &[String]  { &self.config.host.tags }
    fn validate(&self) -> Result<(), FsnError> { self.config.validate() }
}

impl HostResource for HostHandle {
    fn addr(&self)        -> &str  { self.config.host.addr() }
    fn ssh_user(&self)    -> &str  { &self.config.host.ssh_user }
    fn ssh_port(&self)    -> u16   { self.config.host.ssh_port }
    fn is_external(&self) -> bool  { self.config.host.external }
}

// ── Service instance handle ────────────────────────────────────────────────────

/// Runtime wrapper for a loaded service instance config.
///
/// Combines the on-disk `ServiceInstanceConfig` with the filesystem slug and path.
///
/// Implements [`Resource`] and [`ServiceResource`].
#[derive(Debug, Clone)]
pub struct ServiceHandle {
    /// Instance name — derived from `{name}.service.toml` filename stem.
    pub name:      String,
    /// Absolute path to the `.service.toml` file.
    pub toml_path: std::path::PathBuf,
    /// Parsed service instance configuration.
    pub config:    ServiceInstanceConfig,
}

impl Resource for ServiceHandle {
    fn kind(&self) -> &'static str { "service" }
    fn id(&self)   -> &str         { &self.name }
    fn display_name(&self) -> &str {
        self.config.service.alias.as_deref().unwrap_or(&self.name)
    }
    fn tags(&self)  -> &[String]  { &self.config.service.tags }
    fn validate(&self) -> Result<(), FsnError> { self.config.validate() }
}

impl ServiceResource for ServiceHandle {
    fn service_class(&self) -> &str         { &self.config.service.service_class }
    fn host(&self)          -> Option<&str> { self.config.service.host.as_deref() }
    fn subdomain(&self)     -> Option<&str> { self.config.service.subdomain.as_deref() }
    fn port(&self)          -> Option<u16>  { self.config.service.port }
    fn project(&self)       -> &str         { &self.config.service.project }
}

// ── Service table row ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub name:         String,
    pub service_type: String,
    pub domain:       String,
    pub status:       RunState,
}

pub fn run_state_i18n(state: RunState) -> &'static str {
    match state {
        RunState::Running => "status.running",
        RunState::Stopped => "status.stopped",
        RunState::Failed  => "status.error",
        RunState::Missing => "status.unknown",
    }
}

// ── Resource kind ─────────────────────────────────────────────────────────────

pub const PROJECT_TABS: &[&str] = &["form.tab.project", "form.tab.options"];
pub const SERVICE_TABS: &[&str] = &["form.tab.service", "form.tab.network", "form.tab.env"];
pub const HOST_TABS:    &[&str] = &["form.tab.host", "form.tab.system", "form.tab.dns"];
pub const BOT_TABS:     &[&str] = &["form.tab.bot", "form.tab.options"];

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
}

// ── ResourceForm — component-based generic form ───────────────────────────────
//
// Holds a list of `FormNode` objects (TextInputNode, SelectInputNode, …).
// Each node is fully self-contained: it renders itself, handles its own input,
// and knows how to hit-test mouse clicks.
//
// The `on_change` hook receives the nodes slice and the key of the field that
// changed, so smart-default logic (e.g. derive domain from name) can update
// sibling nodes via `node.set_value()`.

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
    /// Returns the action for the outer handler (Submit, Cancel, LangToggle, etc.).
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> FormAction {
        let global_idx = self.focused_node_global_idx();
        let action = if let Some(idx) = global_idx {
            self.nodes[idx].handle_key(key)
        } else {
            FormAction::Unhandled
        };

        // Fire on_change when the value was actually modified
        if action == FormAction::ValueChanged {
            if let Some(idx) = global_idx {
                let key = self.nodes[idx].key();
                (self.on_change)(&mut self.nodes, key);
            }
        }

        // Handle intra-form navigation so the outer handler sees only high-level actions
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

// ── Overlay layer — modal screens above the main UI ───────────────────────────
//
// Implements the "Ebene" (layer) concept: the topmost overlay captures all
// input. Esc pops it. This replaces the old `logs_overlay: Option<LogsState>`
// and `dash_confirm: bool` flags.

#[derive(Debug, Clone)]
pub struct LogsState {
    pub service_name: String,
    pub lines:        Vec<String>,
    pub scroll:       usize,
}

/// Progress message from the background deploy/export thread.
#[derive(Debug)]
pub enum DeployMsg {
    /// Append a line to the deploy log.
    Log(String),
    /// Operation finished (success or failure).
    Done { success: bool, error: Option<String> },
}

/// State for the deploy/export progress overlay.
#[derive(Debug, Clone)]
pub struct DeployState {
    /// What is being deployed or exported (project name).
    pub target:  String,
    /// Log lines shown in the overlay (progress + result).
    pub log:     Vec<String>,
    pub done:    bool,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub enum OverlayLayer {
    Logs(LogsState),
    /// Confirmation prompt. `yes_action` is a tag processed by events.rs.
    Confirm { message: String, yes_action: ConfirmAction },
    /// Deploy / Compose-export progress overlay.
    Deploy(DeployState),
    /// New-resource selector popup (↑↓ to pick, Enter to open form).
    NewResource { selected: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    DeleteProject,
}

/// Options shown in the new-resource selector popup (label key + kind).
/// Order = visual order; do not reorder without updating render.
pub const NEW_RESOURCE_ITEMS: &[(&str, ResourceKind)] = &[
    ("new.project", ResourceKind::Project),
    ("new.host",    ResourceKind::Host),
    ("new.service", ResourceKind::Service),
    ("new.bot",     ResourceKind::Bot),
];

// ── Full application state ────────────────────────────────────────────────────

pub struct AppState {
    pub screen:             Screen,
    pub lang:               Lang,
    pub sysinfo:            SysInfo,
    pub services:           Vec<ServiceRow>,
    pub selected:           usize,
    /// Modal overlay stack — topmost layer gets all input (Ebene system).
    pub overlay_stack:      Vec<OverlayLayer>,
    pub should_quit:        bool,
    /// Whether the F1 help sidebar is visible.
    pub help_visible:       bool,
    pub welcome_focus:      usize,
    pub current_form:       Option<ResourceForm>,
    pub ctrl_hint:          bool,
    pub projects:           Vec<ProjectHandle>,
    pub selected_project:   usize,
    pub hosts:              Vec<HostHandle>,
    pub selected_host:      usize,
    pub svc_handles:        Vec<ServiceHandle>,
    pub dash_focus:         DashFocus,
    /// Flat list of navigable sidebar rows — rebuilt whenever projects or hosts change.
    pub sidebar_items:      Vec<SidebarItem>,
    /// Index into `sidebar_items` — always points to a selectable item.
    pub sidebar_cursor:     usize,
    last_refresh:           Instant,
    last_podman_statuses:   HashMap<String, RunState>,
    /// Receiver for the background deploy/export thread.
    /// `None` when no deploy is running.
    pub deploy_rx:          Option<mpsc::Receiver<DeployMsg>>,
}

impl AppState {
    pub fn new(sysinfo: SysInfo, projects: Vec<ProjectHandle>) -> Self {
        let mut s = Self {
            screen: Screen::Welcome, lang: Lang::De, sysinfo, services: vec![],
            selected: 0, overlay_stack: vec![],
            should_quit: false, help_visible: false, welcome_focus: 0, current_form: None,
            ctrl_hint: false, projects, selected_project: 0,
            hosts: vec![], selected_host: 0, svc_handles: vec![],
            dash_focus: DashFocus::Sidebar,
            sidebar_items: vec![], sidebar_cursor: 0,
            last_refresh: Instant::now(),
            last_podman_statuses: HashMap::new(),
            deploy_rx: None,
        };
        s.rebuild_sidebar();
        s
    }

    // ── Overlay helpers ────────────────────────────────────────────────────

    pub fn push_overlay(&mut self, layer: OverlayLayer) { self.overlay_stack.push(layer); }
    pub fn pop_overlay(&mut self) -> Option<OverlayLayer> { self.overlay_stack.pop() }
    pub fn top_overlay(&self) -> Option<&OverlayLayer> { self.overlay_stack.last() }
    pub fn top_overlay_mut(&mut self) -> Option<&mut OverlayLayer> { self.overlay_stack.last_mut() }
    pub fn has_overlay(&self) -> bool { !self.overlay_stack.is_empty() }

    /// Shortcut — is the topmost overlay a Logs panel?
    pub fn logs_overlay(&self) -> Option<&LogsState> {
        self.overlay_stack.last().and_then(|o| {
            if let OverlayLayer::Logs(ref l) = o { Some(l) } else { None }
        })
    }
    pub fn logs_overlay_mut(&mut self) -> Option<&mut LogsState> {
        self.overlay_stack.last_mut().and_then(|o| {
            if let OverlayLayer::Logs(ref mut l) = o { Some(l) } else { None }
        })
    }

    /// Shortcut — is the topmost overlay a Confirm dialog?
    pub fn confirm_overlay(&self) -> Option<(&str, ConfirmAction)> {
        self.overlay_stack.last().and_then(|o| {
            if let OverlayLayer::Confirm { message, yes_action } = o {
                Some((message.as_str(), *yes_action))
            } else {
                None
            }
        })
    }

    // ── Podman / service helpers ───────────────────────────────────────────

    pub fn apply_podman_status(&mut self, statuses: HashMap<String, RunState>) {
        self.last_podman_statuses = statuses;
        self.rebuild_services();
    }

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
        if self.selected >= self.services.len() && !self.services.is_empty() {
            self.selected = self.services.len() - 1;
        }
    }

    /// Rebuild the flat sidebar item list from current `projects` and `hosts`.
    ///
    /// Preserves the cursor position where possible; clamps and advances past
    /// non-selectable items (section headers) automatically.
    /// Call this whenever `projects` or `hosts` change.
    pub fn rebuild_sidebar(&mut self) {
        let prev = self.sidebar_cursor;

        let mut items: Vec<SidebarItem> = vec![SidebarItem::Section("sidebar.projects")];
        for p in &self.projects {
            items.push(SidebarItem::Project {
                slug: p.slug.clone(),
                name: p.config.project.name.clone(),
            });
        }
        items.push(SidebarItem::Action { label_key: "dash.new_project", kind: SidebarAction::NewProject });

        items.push(SidebarItem::Section("sidebar.hosts"));
        for h in &self.hosts {
            items.push(SidebarItem::Host {
                slug: h.slug.clone(),
                name: h.display_name().to_string(),
            });
        }
        items.push(SidebarItem::Action { label_key: "dash.new_host", kind: SidebarAction::NewHost });

        self.sidebar_items = items;

        // Clamp; if the clamped index is non-selectable, advance to the next selectable item.
        let clamped = prev.min(self.sidebar_items.len().saturating_sub(1));
        self.sidebar_cursor = if self.sidebar_items.get(clamped).map(|i| i.is_selectable()).unwrap_or(false) {
            clamped
        } else {
            self.sidebar_items.iter().position(|i| i.is_selectable()).unwrap_or(0)
        };
    }

    /// The sidebar item currently pointed to by the cursor, if any.
    pub fn current_sidebar_item(&self) -> Option<&SidebarItem> {
        self.sidebar_items.get(self.sidebar_cursor)
    }

    pub fn t<'a>(&self, key: &'a str) -> &'a str {
        crate::i18n::t(self.lang, key)
    }

    /// Mutable access to the deploy overlay (if it's on top).
    pub fn deploy_overlay_mut(&mut self) -> Option<&mut DeployState> {
        self.overlay_stack.last_mut().and_then(|o| {
            if let OverlayLayer::Deploy(ref mut d) = o { Some(d) } else { None }
        })
    }

    /// Apply a message from the background deploy thread.
    pub fn apply_deploy_msg(&mut self, msg: DeployMsg) {
        if let Some(ds) = self.deploy_overlay_mut() {
            match msg {
                DeployMsg::Log(line) => ds.log.push(line),
                DeployMsg::Done { success, error } => {
                    if let Some(err) = error {
                        ds.log.push(format!("✗ {}", err));
                    } else {
                        ds.log.push("✓ Fertig!".into());
                    }
                    ds.done    = true;
                    ds.success = success;
                    self.deploy_rx = None;
                }
            }
        }
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
                Event::Key(key)   => crate::events::handle(key, state, root)?,
                Event::Mouse(mouse) => crate::events::handle_mouse(mouse, state)?,
                _ => {}
            }
        }

        if state.should_quit { break; }

        while let Ok(statuses) = reconcile_rx.try_recv() {
            state.apply_podman_status(statuses);
        }

        // Poll deploy/export progress messages from background thread
        if let Some(ref rx) = state.deploy_rx {
            let msgs: Vec<DeployMsg> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
            for msg in msgs { state.apply_deploy_msg(msg); }
        }

        if state.last_refresh.elapsed() >= Duration::from_secs(REFRESH_SECS) {
            state.sysinfo = SysInfo::collect();
            state.last_refresh = Instant::now();
        }
    }

    Ok(())
}
