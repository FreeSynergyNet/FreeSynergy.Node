// Application state and main event loop.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ratatui::layout::Rect;

use fsn_core::config::AppSettings;
use fsn_core::health::{self, HealthLevel};
use fsn_core::resource::Resource;
use fsn_core::store::StoreEntry;

use crate::click_map::ClickMap;
use crate::sysinfo::SysInfo;

// Re-export all handle and form types so existing `use crate::app::*` imports keep working.
pub use crate::handles::{HostHandle, ProjectHandle, RunState, ServiceHandle, ServiceRow, run_state_i18n};
pub use crate::resource_form::{
    BOT_TABS, HOST_TABS, PROJECT_TABS, ResourceForm, ResourceKind, SERVICE_TABS, slugify,
};

// ── Notifications (toast system) ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifKind { Success, Warning, Error, Info }

#[derive(Debug, Clone)]
pub struct Notification {
    pub message:   String,
    pub kind:      NotifKind,
    pub born:      Instant,
    /// Tick at creation — used by Anim::notif_width() for slide-in effect.
    pub born_tick: u32,
}

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
    /// A project entry — includes a pre-computed health level for the sidebar indicator.
    Project { slug: String, name: String, health: HealthLevel },
    /// A host entry — includes a pre-computed health level for the sidebar indicator.
    Host    { slug: String, name: String, health: HealthLevel },
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

    /// Context menu actions available for this item type.
    ///
    /// Single source of truth — to add/remove actions per type: edit only here.
    /// Called by mouse.rs right-click handler; no duplicate lists anywhere else.
    pub fn context_actions(&self) -> Vec<ContextAction> {
        match self {
            SidebarItem::Project { .. } => vec![
                ContextAction::Edit,
                ContextAction::AddService,
                ContextAction::AddHost,
                ContextAction::Deploy,
                ContextAction::Delete,
            ],
            SidebarItem::Host { .. } => vec![
                ContextAction::Edit,
                ContextAction::Deploy,
                ContextAction::Delete,
            ],
            SidebarItem::Service { status, .. } => {
                let start_stop = if *status == RunState::Running { ContextAction::Stop } else { ContextAction::Start };
                vec![start_stop, ContextAction::Logs, ContextAction::Edit, ContextAction::Delete]
            }
            _ => vec![],
        }
    }

    /// Confirm-overlay parameters for deleting this item, if applicable.
    ///
    /// Returns `(message_key, optional_data, yes_action)`.
    /// Used by `execute_context_action` — add new resource types here only.
    pub fn delete_confirm(&self) -> Option<(String, Option<String>, ConfirmAction)> {
        match self {
            SidebarItem::Project { .. } =>
                Some(("confirm.delete.project".into(), None, ConfirmAction::DeleteProject)),
            SidebarItem::Host { slug, .. } =>
                Some(("confirm.delete.host".into(), Some(slug.clone()), ConfirmAction::DeleteHost)),
            SidebarItem::Service { name, .. } =>
                Some(("confirm.delete.service".into(), Some(name.clone()), ConfirmAction::DeleteService)),
            _ => None,
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
    ContextMenu,
}

#[derive(Debug, Clone)]
pub enum OverlayLayer {
    Logs(LogsState),
    Confirm { message: String, data: Option<String>, yes_action: ConfirmAction },
    Deploy(DeployState),
    NewResource { selected: usize },
    /// Right-click context menu — rendered at (x, y), navigated with ↑↓/Enter/Esc.
    /// `source` carries the item that was right-clicked; `None` for generic menus (e.g. 'n').
    ContextMenu { x: u16, y: u16, items: Vec<ContextAction>, selected: usize, source: Option<ActionSource> },
}

impl OverlayLayer {
    /// Returns the discriminant without borrowing the inner data.
    pub fn kind(&self) -> OverlayKind {
        match self {
            OverlayLayer::Logs(_)            => OverlayKind::Logs,
            OverlayLayer::Confirm { .. }     => OverlayKind::Confirm,
            OverlayLayer::Deploy(_)          => OverlayKind::Deploy,
            OverlayLayer::NewResource { .. } => OverlayKind::NewResource,
            OverlayLayer::ContextMenu { .. } => OverlayKind::ContextMenu,
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

/// Who triggered a context menu — carried inside `OverlayLayer::ContextMenu`.
///
/// Design Pattern: Single Source of Truth for context dispatch.
/// Storing the source at click-time means `execute_context_action` never has
/// to infer the item from the current sidebar/focus state.
/// Rule: add variants here if a new clickable area gets its own context menu.
#[derive(Debug, Clone)]
pub enum ActionSource {
    /// A sidebar item was right-clicked.
    Sidebar(SidebarItem),
}

// ── Context menu actions — right-click menu ───────────────────────────────────
//
// Design: ContextAction is the single source for which actions exist and what
// they're called. mouse.rs decides which actions apply per item type.
// events.rs executes the selected action. i18n keys follow "ctx.*" prefix.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction { Edit, Delete, Deploy, Start, Stop, Logs, AddService, AddHost }

impl ContextAction {
    /// i18n key for this action's label.
    pub fn label_key(self) -> &'static str {
        match self {
            ContextAction::Edit       => "ctx.edit",
            ContextAction::Delete     => "ctx.delete",
            ContextAction::Deploy     => "ctx.deploy",
            ContextAction::Start      => "ctx.start",
            ContextAction::Stop       => "ctx.stop",
            ContextAction::Logs       => "ctx.logs",
            ContextAction::AddService => "ctx.add_service",
            ContextAction::AddHost    => "ctx.add_host",
        }
    }

    /// Danger actions render in red.
    pub fn is_danger(self) -> bool {
        matches!(self, ContextAction::Delete | ContextAction::Stop)
    }
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
    /// Background reconciler — receives Podman container status maps every ~5 s.
    pub reconcile_rx:         Option<mpsc::Receiver<HashMap<String, RunState>>>,
    /// Background store fetcher — receives fresh entries after HTTP fetch completes.
    pub store_rx:             Option<mpsc::Receiver<Vec<fsn_core::store::StoreEntry>>>,
    pub task_queue:           Option<crate::task_queue::TaskQueue>,
    pub settings:             AppSettings,
    pub store_entries:        Vec<StoreEntry>,
    pub settings_cursor:      usize,
    /// Non-blocking feedback banners (auto-expire after a few seconds).
    pub notifications:        Vec<Notification>,
    /// Active sidebar filter query — `None` = closed, `Some("")` = open but empty.
    pub sidebar_filter:       Option<String>,
    /// Indices of services currently selected for batch operations.
    /// Empty = no multi-select mode active.
    pub selected_services:    HashSet<usize>,

    // ── Animation ─────────────────────────────────────────────────────────────
    /// Tick-driven animation state — advanced once per 250ms tick.
    pub anim: crate::ui::anim::Anim,

    // ── Mouse support ─────────────────────────────────────────────────────────
    /// Last left-click position + time — used for double-click detection.
    pub last_click: Option<(u16, u16, Instant)>,
    /// Cached sidebar list area (set during render) — used for mouse hit-testing.
    pub sidebar_list_area: Option<Rect>,
    /// Cached services table area (set during render) — used for mouse hit-testing.
    pub services_table_area: Option<Rect>,
    /// Per-frame registry of all clickable UI elements.
    /// Cleared and rebuilt by each screen's render function.
    /// Mouse dispatch queries this once — no per-screen if/else.
    pub click_map: ClickMap,
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
            reconcile_rx: None,
            store_rx: None,
            task_queue: None,
            settings: AppSettings::load().unwrap_or_default(),
            store_entries: Vec::new(),
            settings_cursor: 0,
            notifications: Vec::new(),
            sidebar_filter: None,
            selected_services: HashSet::new(),
            anim: crate::ui::anim::Anim::new(),
            last_click: None,
            sidebar_list_area: None,
            services_table_area: None,
            click_map: ClickMap::new(),
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

        // Pre-collect host→project assignments for cross-resource health checks.
        let host_projects: Vec<&str> = self.hosts.iter()
            .filter_map(|h| h.config.host.project.as_deref())
            .collect();

        let mut items: Vec<SidebarItem> = vec![SidebarItem::Section("sidebar.projects")];
        for p in &self.projects {
            let h = health::check_project(&p.config, &host_projects);
            items.push(SidebarItem::Project {
                slug:   p.slug.clone(),
                name:   p.config.project.name.clone(),
                health: h.overall,
            });
        }
        items.push(SidebarItem::Action { label_key: "dash.new_project", kind: SidebarAction::NewProject });

        items.push(SidebarItem::Section("sidebar.hosts"));
        for h in &self.hosts {
            let hs = health::check_host(&h.config);
            items.push(SidebarItem::Host {
                slug:   h.slug.clone(),
                name:   h.display_name().to_string(),
                health: hs.overall,
            });
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

    // ── Notification helpers ───────────────────────────────────────────────

    pub fn push_notif(&mut self, kind: NotifKind, message: impl Into<String>) {
        let born_tick = self.anim.tick();
        self.notifications.push(Notification {
            message: message.into(),
            kind,
            born: Instant::now(),
            born_tick,
        });
    }

    /// Remove notifications older than `max_age`. Called each loop tick.
    pub fn expire_notifications(&mut self, max_age: Duration) {
        self.notifications.retain(|n| n.born.elapsed() < max_age);
    }

    /// Sidebar items visible given the current filter query.
    /// Sections and Action items are hidden while a non-empty filter is active.
    pub fn visible_sidebar_items(&self) -> Vec<(usize, &SidebarItem)> {
        let filter = self.sidebar_filter.as_deref().unwrap_or("").to_lowercase();
        self.sidebar_items.iter().enumerate()
            .filter(|(_, item)| {
                if filter.is_empty() { return true; }
                match item {
                    SidebarItem::Project { name, .. } => name.to_lowercase().contains(&filter),
                    SidebarItem::Host    { name, .. } => name.to_lowercase().contains(&filter),
                    SidebarItem::Service { name, .. } => name.to_lowercase().contains(&filter),
                    _ => false,
                }
            })
            .collect()
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

// ── rat-salsa event loop ──────────────────────────────────────────────────────
//
// Design: thin wrappers that forward to the existing event/render modules.
// AppGlobal carries root path so fn-pointer callbacks can access it.
// AppEvent wraps crossterm events + a periodic tick for channel polling.

use std::path::PathBuf;

use rat_salsa::{Control, RunConfig, SalsaAppContext, SalsaContext, run_tui};
use rat_salsa::poll::{PollCrossterm, PollTimers};
use rat_salsa::timer::{TimeOut, TimerDef};

/// Events dispatched by rat-salsa to the application.
#[derive(Debug)]
pub enum AppEvent {
    /// Raw crossterm event (keyboard, resize, etc.).
    Crossterm(crossterm::event::Event),
    /// Periodic tick — polls background channels and refreshes sysinfo.
    Tick(TimeOut),
}

impl From<crossterm::event::Event> for AppEvent {
    fn from(e: crossterm::event::Event) -> Self { AppEvent::Crossterm(e) }
}

impl From<TimeOut> for AppEvent {
    fn from(t: TimeOut) -> Self { AppEvent::Tick(t) }
}

/// Global state accessible from all rat-salsa callbacks (init/render/event/error).
pub struct AppGlobal {
    ctx:  SalsaAppContext<AppEvent, anyhow::Error>,
    /// Root path of the FSN workspace — forwarded to events::handle.
    pub root: PathBuf,
}

impl SalsaContext<AppEvent, anyhow::Error> for AppGlobal {
    fn set_salsa_ctx(&mut self, app_ctx: SalsaAppContext<AppEvent, anyhow::Error>) {
        self.ctx = app_ctx;
    }
    fn salsa_ctx(&self) -> &SalsaAppContext<AppEvent, anyhow::Error> { &self.ctx }
}

/// Entry point for the rat-salsa event loop. Called from `lib.rs::run()`.
/// Terminal setup (raw mode, alternate screen, mouse capture) is handled by rat-salsa.
pub fn run_salsa(root: PathBuf, state: &mut AppState) -> anyhow::Result<()> {
    let mut global = AppGlobal { ctx: Default::default(), root };
    run_tui(
        fsn_init,
        fsn_render,
        fsn_event,
        fsn_error,
        &mut global,
        state,
        RunConfig::default()?.poll(PollCrossterm).poll(PollTimers::new()),
    )?;
    Ok(())
}

fn fsn_init(state: &mut AppState, ctx: &mut AppGlobal) -> anyhow::Result<()> {
    // Repeating 250 ms tick — used to drain background mpsc channels.
    ctx.add_timer(TimerDef::new().repeat_forever().timer(Duration::from_millis(250)));
    // Force an immediate render so the screen isn't blank before the first event.
    let _ = state;
    Ok(())
}

fn fsn_render(
    area:  ratatui::layout::Rect,
    buf:   &mut ratatui::buffer::Buffer,
    state: &mut AppState,
    _ctx:  &mut AppGlobal,
) -> anyhow::Result<()> {
    let mut rctx = crate::ui::render_ctx::RenderCtx::new(area, buf);
    crate::ui::render(&mut rctx, state);
    Ok(())
}

fn fsn_event(
    event: &AppEvent,
    state: &mut AppState,
    ctx:   &mut AppGlobal,
) -> anyhow::Result<Control<AppEvent>> {
    match event {
        AppEvent::Crossterm(e) => {
            match e {
                crossterm::event::Event::Key(key) => {
                    crate::events::handle(*key, state, ctx.root.as_path())?;
                }
                crossterm::event::Event::Mouse(mouse) => {
                    crate::mouse::handle_mouse(*mouse, state, ctx.root.as_path())?;
                }
                _ => {}
            }
            if state.should_quit { return Ok(Control::Quit); }
            Ok(Control::Changed)
        }

        AppEvent::Tick(_) => {
            // Drain reconciler channel — collect first to avoid simultaneous borrow.
            let reconcile_msgs: Vec<HashMap<String, RunState>> = state.reconcile_rx
                .as_ref()
                .map(|rx| std::iter::from_fn(|| rx.try_recv().ok()).collect())
                .unwrap_or_default();
            for statuses in reconcile_msgs { state.apply_podman_status(statuses); }

            // Drain deploy channel.
            let deploy_msgs: Vec<DeployMsg> = state.deploy_rx
                .as_ref()
                .map(|rx| std::iter::from_fn(|| rx.try_recv().ok()).collect())
                .unwrap_or_default();
            for msg in deploy_msgs { state.apply_deploy_msg(msg); }

            // Drain store fetcher channel (one-shot).
            let store_result: Option<Vec<StoreEntry>> = state.store_rx
                .as_ref()
                .and_then(|rx| rx.try_recv().ok());
            if let Some(entries) = store_result {
                let count = entries.len();
                state.store_entries = entries;
                state.store_rx = None;
                if count > 0 {
                    state.push_notif(NotifKind::Info, format!("Store: {count} Module geladen"));
                }
            }

            // Refresh sysinfo every 5 s.
            if state.last_refresh.elapsed() >= Duration::from_secs(5) {
                state.sysinfo = crate::sysinfo::SysInfo::collect();
                state.last_refresh = Instant::now();
            }

            state.expire_notifications(Duration::from_secs(4));
            state.anim.advance();
            Ok(Control::Changed)
        }
    }
}

fn fsn_error(
    err:    anyhow::Error,
    _state: &mut AppState,
    _ctx:   &mut AppGlobal,
) -> anyhow::Result<Control<AppEvent>> {
    tracing::error!("{:#}", err);
    Ok(Control::Changed)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sysinfo::SysInfo;
    use std::time::Duration;

    fn empty_state() -> AppState {
        AppState::new(SysInfo::default(), vec![])
    }

    // ── SidebarItem ───────────────────────────────────────────────────────

    #[test]
    fn section_is_not_selectable() {
        let item = SidebarItem::Section("sidebar.projects");
        assert!(!item.is_selectable());
    }

    #[test]
    fn project_is_selectable() {
        let item = SidebarItem::Project { slug: "p".into(), name: "My Project".into(), health: HealthLevel::Ok };
        assert!(item.is_selectable());
    }

    #[test]
    fn action_kind_returns_correct_variant() {
        let item = SidebarItem::Action { label_key: "dash.new_project", kind: SidebarAction::NewProject };
        assert_eq!(item.action_kind(), Some(SidebarAction::NewProject));
    }

    #[test]
    fn section_action_kind_is_none() {
        let item = SidebarItem::Section("sidebar.hosts");
        assert_eq!(item.action_kind(), None);
    }

    #[test]
    fn hint_key_host() {
        let item = SidebarItem::Host { slug: "h".into(), name: "srv1".into(), health: HealthLevel::Ok };
        assert_eq!(item.hint_key(), "dash.hint.host");
    }

    #[test]
    fn hint_key_service() {
        let item = SidebarItem::Service { name: "kanidm".into(), class: "iam".into(), status: RunState::Running };
        assert_eq!(item.hint_key(), "dash.hint.service");
    }

    // ── Notifications ─────────────────────────────────────────────────────

    #[test]
    fn push_notif_appends() {
        let mut state = empty_state();
        state.push_notif(NotifKind::Success, "Saved");
        assert_eq!(state.notifications.len(), 1);
        assert_eq!(state.notifications[0].message, "Saved");
        assert_eq!(state.notifications[0].kind, NotifKind::Success);
    }

    #[test]
    fn expire_notifications_removes_old() {
        let mut state = empty_state();
        state.push_notif(NotifKind::Info, "old");
        // Manually set born to 10s ago by sleeping 0ms + using max_age=0
        state.expire_notifications(Duration::from_millis(0));
        // After expiry with 0ms max_age, all are removed
        assert!(state.notifications.is_empty());
    }

    #[test]
    fn expire_notifications_keeps_fresh() {
        let mut state = empty_state();
        state.push_notif(NotifKind::Info, "fresh");
        state.expire_notifications(Duration::from_secs(60));
        assert_eq!(state.notifications.len(), 1);
    }

    // ── Sidebar filter ────────────────────────────────────────────────────

    #[test]
    fn visible_sidebar_items_no_filter() {
        let mut state = empty_state();
        state.sidebar_items = vec![
            SidebarItem::Section("sidebar.projects"),
            SidebarItem::Project { slug: "p".into(), name: "Alpha".into(), health: HealthLevel::Ok },
        ];
        state.sidebar_filter = None;
        let visible = state.visible_sidebar_items();
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn visible_sidebar_items_filter_matches() {
        let mut state = empty_state();
        state.sidebar_items = vec![
            SidebarItem::Section("sidebar.projects"),
            SidebarItem::Project { slug: "alpha".into(), name: "Alpha".into(), health: HealthLevel::Ok },
            SidebarItem::Project { slug: "beta".into(),  name: "Beta".into(),  health: HealthLevel::Ok },
        ];
        state.sidebar_filter = Some("alp".into());
        let visible = state.visible_sidebar_items();
        assert_eq!(visible.len(), 1);
        assert!(matches!(&visible[0].1, SidebarItem::Project { slug, .. } if slug == "alpha"));
    }

    #[test]
    fn visible_sidebar_items_filter_case_insensitive() {
        let mut state = empty_state();
        state.sidebar_items = vec![
            SidebarItem::Project { slug: "p".into(), name: "MyApp".into(), health: HealthLevel::Ok },
        ];
        state.sidebar_filter = Some("myapp".into());
        assert_eq!(state.visible_sidebar_items().len(), 1);
    }

    #[test]
    fn visible_sidebar_items_filter_no_match() {
        let mut state = empty_state();
        state.sidebar_items = vec![
            SidebarItem::Project { slug: "p".into(), name: "Alpha".into(), health: HealthLevel::Ok },
        ];
        state.sidebar_filter = Some("zzz".into());
        assert!(state.visible_sidebar_items().is_empty());
    }
}
