// Application state and main event loop.

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use fsn_core::config::AppSettings;
use fsn_core::resource::Resource;
use fsn_core::store::StoreEntry;

use crate::sysinfo::SysInfo;
use crate::ui;

// Re-export all handle and form types so existing `use crate::app::*` imports keep working.
pub use crate::handles::{HostHandle, ProjectHandle, RunState, ServiceHandle, ServiceRow, run_state_i18n};
pub use crate::resource_form::{
    BOT_TABS, HOST_TABS, PROJECT_TABS, ResourceForm, ResourceKind, SERVICE_TABS, slugify,
};

// ── Screens ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Dashboard,
    NewProject,
    /// Progressive setup wizard — task queue with per-task save.
    TaskWizard,
    /// Application settings — store management, preferences.
    Settings,
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
pub enum SidebarAction { NewProject, NewHost, NewService }

/// One navigable row in the sidebar.
///
/// Analogous to a DOM element — each variant knows its own visual appearance
/// and the action it triggers when selected.
#[derive(Debug, Clone)]
pub enum SidebarItem {
    Section(&'static str),
    Project { slug: String, name: String },
    Host    { slug: String, name: String },
    Service { name: String, class: String, status: RunState },
    Action  { label_key: &'static str, kind: SidebarAction },
}

impl SidebarItem {
    pub fn is_selectable(&self) -> bool {
        !matches!(self, SidebarItem::Section(_))
    }
    pub fn action_kind(&self) -> Option<SidebarAction> {
        if let SidebarItem::Action { kind, .. } = self { Some(*kind) } else { None }
    }
    pub fn hint_key(&self) -> &'static str {
        match self {
            SidebarItem::Host    { .. } => "dash.hint.host",
            SidebarItem::Service { .. } => "dash.hint.service",
            _                           => "dash.hint",
        }
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

// ── Overlay layer — modal screens above the main UI ───────────────────────────
//
// Implements the "Ebene" (layer) concept: the topmost overlay captures all input.

#[derive(Debug, Clone)]
pub struct LogsState {
    pub service_name: String,
    pub lines:        Vec<String>,
    pub scroll:       usize,
}

/// Progress message from the background deploy/export thread.
#[derive(Debug)]
pub enum DeployMsg {
    Log(String),
    Done { success: bool, error: Option<String> },
}

/// State for the deploy/export progress overlay.
#[derive(Debug, Clone)]
pub struct DeployState {
    pub target:  String,
    pub log:     Vec<String>,
    pub done:    bool,
    pub success: bool,
}

/// Discriminant for the overlay variant — used for type-safe dispatch in event handlers.
/// Avoids string-matching while still allowing borrow-safe inspection (reading kind,
/// then taking a mutable borrow separately for the actual handling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Logs,
    Confirm,
    Deploy,
    NewResource,
}

#[derive(Debug, Clone)]
pub enum OverlayLayer {
    Logs(LogsState),
    Confirm { message: String, data: Option<String>, yes_action: ConfirmAction },
    Deploy(DeployState),
    NewResource { selected: usize },
}

impl OverlayLayer {
    /// Returns the discriminant without borrowing the inner data.
    pub fn kind(&self) -> OverlayKind {
        match self {
            OverlayLayer::Logs(_)          => OverlayKind::Logs,
            OverlayLayer::Confirm { .. }   => OverlayKind::Confirm,
            OverlayLayer::Deploy(_)        => OverlayKind::Deploy,
            OverlayLayer::NewResource { .. } => OverlayKind::NewResource,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    DeleteProject,
    DeleteService,
    DeleteHost,
    StopService,
    LeaveForm,
    LeaveWizard,
    Quit,
}

/// Options shown in the new-resource selector popup (label key + kind).
pub const NEW_RESOURCE_ITEMS: &[(&str, ResourceKind)] = &[
    ("new.project", ResourceKind::Project),
    ("new.host",    ResourceKind::Host),
    ("new.service", ResourceKind::Service),
    ("new.bot",     ResourceKind::Bot),
];

// ── Full application state ────────────────────────────────────────────────────

pub struct AppState {
    pub screen:               Screen,
    pub lang:                 Lang,
    pub sysinfo:              SysInfo,
    pub services:             Vec<ServiceRow>,
    pub selected:             usize,
    pub overlay_stack:        Vec<OverlayLayer>,
    pub should_quit:          bool,
    pub help_visible:         bool,
    pub welcome_focus:        usize,
    pub current_form:         Option<ResourceForm>,
    pub ctrl_hint:            bool,
    pub projects:             Vec<ProjectHandle>,
    pub selected_project:     usize,
    pub hosts:                Vec<HostHandle>,
    pub selected_host:        usize,
    pub svc_handles:          Vec<ServiceHandle>,
    pub dash_focus:           DashFocus,
    pub sidebar_items:        Vec<SidebarItem>,
    pub sidebar_cursor:       usize,
    last_refresh:             Instant,
    pub last_podman_statuses: HashMap<String, RunState>,
    pub deploy_rx:            Option<mpsc::Receiver<DeployMsg>>,
    pub task_queue:           Option<crate::task_queue::TaskQueue>,
    pub settings:             AppSettings,
    pub store_entries:        Vec<StoreEntry>,
    pub settings_cursor:      usize,
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
            task_queue: None,
            settings: AppSettings::load().unwrap_or_default(),
            store_entries: Vec::new(),
            settings_cursor: 0,
        };
        s.rebuild_sidebar();
        s
    }

    pub fn store_entries_for_type(&self, service_type: &str) -> Vec<&StoreEntry> {
        self.store_entries.iter()
            .filter(|e| e.service_type == service_type)
            .collect()
    }

    pub fn class_options_for_type(&self, service_type: &str, local_default: &str) -> Vec<String> {
        let mut opts: Vec<String> = vec![local_default.to_string()];
        for entry in self.store_entries_for_type(service_type) {
            if entry.id != local_default {
                opts.push(entry.id.clone());
            }
        }
        opts
    }

    // ── Overlay helpers ────────────────────────────────────────────────────

    pub fn push_overlay(&mut self, layer: OverlayLayer) { self.overlay_stack.push(layer); }
    pub fn pop_overlay(&mut self) -> Option<OverlayLayer> { self.overlay_stack.pop() }
    pub fn top_overlay(&self) -> Option<&OverlayLayer> { self.overlay_stack.last() }
    pub fn top_overlay_mut(&mut self) -> Option<&mut OverlayLayer> { self.overlay_stack.last_mut() }
    pub fn has_overlay(&self) -> bool { !self.overlay_stack.is_empty() }

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

    pub fn confirm_overlay(&self) -> Option<(&str, Option<&str>, ConfirmAction)> {
        self.overlay_stack.last().and_then(|o| {
            if let OverlayLayer::Confirm { message, data, yes_action } = o {
                Some((message.as_str(), data.as_deref(), *yes_action))
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

    pub fn rebuild_sidebar(&mut self) {
        let prev = self.sidebar_cursor;

        let mut items: Vec<SidebarItem> = vec![SidebarItem::Section("sidebar.projects")];
        for p in &self.projects {
            items.push(SidebarItem::Project { slug: p.slug.clone(), name: p.config.project.name.clone() });
        }
        items.push(SidebarItem::Action { label_key: "dash.new_project", kind: SidebarAction::NewProject });

        items.push(SidebarItem::Section("sidebar.hosts"));
        for h in &self.hosts {
            items.push(SidebarItem::Host { slug: h.slug.clone(), name: h.display_name().to_string() });
        }
        items.push(SidebarItem::Action { label_key: "dash.new_host", kind: SidebarAction::NewHost });

        items.push(SidebarItem::Section("sidebar.services"));
        if let Some(proj) = self.projects.get(self.selected_project) {
            for (name, entry) in &proj.config.load.services {
                let status = self.last_podman_statuses.get(name).copied().unwrap_or(RunState::Missing);
                items.push(SidebarItem::Service { name: name.clone(), class: entry.service_class.clone(), status });
            }
        }
        items.push(SidebarItem::Action { label_key: "dash.new_service", kind: SidebarAction::NewService });

        self.sidebar_items = items;

        let clamped = prev.min(self.sidebar_items.len().saturating_sub(1));
        self.sidebar_cursor = if self.sidebar_items.get(clamped).map(|i| i.is_selectable()).unwrap_or(false) {
            clamped
        } else {
            self.sidebar_items.iter().position(|i| i.is_selectable()).unwrap_or(0)
        };
    }

    pub fn current_sidebar_item(&self) -> Option<&SidebarItem> {
        self.sidebar_items.get(self.sidebar_cursor)
    }

    pub fn t<'a>(&self, key: &'a str) -> &'a str {
        crate::i18n::t(self.lang, key)
    }

    pub fn deploy_overlay_mut(&mut self) -> Option<&mut DeployState> {
        self.overlay_stack.last_mut().and_then(|o| {
            if let OverlayLayer::Deploy(ref mut d) = o { Some(d) } else { None }
        })
    }

    pub fn apply_deploy_msg(&mut self, msg: DeployMsg) {
        if let Some(ds) = self.deploy_overlay_mut() {
            match msg {
                DeployMsg::Log(line) => ds.log.push(line),
                DeployMsg::Done { success, error } => {
                    if let Some(err) = error { ds.log.push(format!("✗ {}", err)); }
                    else { ds.log.push("✓ Fertig!".into()); }
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
                Event::Key(key)     => crate::events::handle(key, state, root)?,
                Event::Mouse(mouse) => crate::mouse::handle_mouse(mouse, state, root)?,
                _ => {}
            }
        }

        if state.should_quit { break; }

        while let Ok(statuses) = reconcile_rx.try_recv() {
            state.apply_podman_status(statuses);
        }

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
