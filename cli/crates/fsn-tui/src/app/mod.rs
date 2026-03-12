// Application state — root module for the app/ package.
//
// Design Pattern: Facade — mod.rs re-exports all sub-modules so that existing
// `use crate::app::XYZ` imports continue to work without modification.
//
// Sub-modules:
//   event_loop.rs — AppEvent, AppGlobal, run_salsa + rat-salsa callbacks
//   lang.rs       — Lang enum + toggle logic
//   notif.rs      — Notification, NotifKind (toast system)
//   overlay.rs    — OverlayLayer, OverlayKind, ConfirmAction, ContextAction, ActionSource
//   screen.rs     — Screen enum, DashFocus enum
//   sidebar.rs    — SidebarItem enum + impl blocks, SidebarAction

pub mod event_loop;
pub mod lang;
pub mod notif;
pub mod overlay;
pub mod screen;
pub mod sidebar;

pub use event_loop::{AppEvent, AppGlobal, run_salsa};

// ── Flat re-exports (preserve existing `use crate::app::XYZ` imports) ─────────

pub use lang::Lang;
pub use notif::{Notification, NotifKind};
pub use overlay::{
    ActionSource, ConfirmAction, ContextAction, DeployMsg, DeployState,
    LogsState, OverlayKind, OverlayLayer,
};
pub use screen::{DashFocus, Screen, SettingsFocus, SettingsSection, SettingsTab};
pub use sidebar::{NEW_RESOURCE_ITEMS, SidebarAction, SidebarItem};

// Re-export handle and form types so existing `use crate::app::*` imports keep working.
pub use crate::handles::{HostHandle, ProjectHandle, RunState, ServiceHandle, ServiceRow, run_state_i18n};
pub use crate::resource_form::{
    BOT_TABS, HOST_TABS, PROJECT_TABS, ResourceForm, ResourceKind, SERVICE_TABS, STORE_TABS, slugify,
};

// ── Full application state ────────────────────────────────────────────────────

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ratatui::layout::Rect;

use fsn_core::config::AppSettings;
use fsn_core::health;
use fsn_core::Resource;
use fsn_core::store::StoreEntry;

use crate::click_map::ClickMap;
use crate::i18n::DynamicLang;
use crate::sysinfo::SysInfo;

pub struct AppState {
    pub screen:               Screen,
    pub lang:                 Lang,
    /// All languages loaded from ~/.local/share/fsn/i18n/ at startup.
    pub available_langs:      Vec<&'static DynamicLang>,
    pub sysinfo:              SysInfo,
    pub services:             Vec<ServiceRow>,
    pub selected:             usize,
    pub overlay_stack:        Vec<OverlayLayer>,
    pub should_quit:          bool,
    pub help_visible:         bool,
    pub welcome_focus:        usize,
    /// Unified form queue — replaces `current_form` and the old `task_queue`.
    /// `None` when no form is open. `Some` when one or more forms are queued.
    pub form_queue:           Option<crate::form_queue::FormQueue>,
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
    /// Background language downloader — receives Ok(code) on success, Err(msg) on failure.
    pub lang_download_rx:     Option<mpsc::Receiver<Result<String, String>>>,
    pub settings:                AppSettings,
    pub store_entries:           Vec<StoreEntry>,
    /// Cursor within the Settings content panel (stores list position).
    pub settings_cursor:         usize,
    /// Active settings section — drives both sidebar highlight and content panel.
    pub settings_section:        SettingsSection,
    /// Which side of the Settings screen has keyboard focus.
    pub settings_focus:          SettingsFocus,
    /// Cursor within the Settings sidebar (0=Stores, 1=Languages, 2=General, 3=About).
    pub settings_sidebar_cursor: usize,
    /// Cursor within the Languages section content list.
    pub lang_cursor:             usize,
    /// Language entries fetched from the Store (Node/i18n/index.toml).
    pub store_langs:             Vec<crate::StoreLangEntry>,
    /// Background fetcher for store language index (one-shot).
    /// Sends Ok(entries) on success or Err(message) on failure.
    pub store_langs_rx:          Option<mpsc::Receiver<Result<Vec<crate::StoreLangEntry>, String>>>,
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
        let i18n_dir = {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            std::path::PathBuf::from(home).join(".local/share/fsn/i18n")
        };
        let available_langs = DynamicLang::load_dir(&i18n_dir);
        let settings = AppSettings::load().unwrap_or_default();

        // Language priority: saved setting → system locale → English.
        let preferred_code: Option<String> = settings.preferred_lang.clone()
            .or_else(system_lang_code);
        let lang = preferred_code.as_deref()
            .and_then(|code| available_langs.iter().find(|d| d.code == code))
            .map(|d| Lang::Dynamic(d))
            .unwrap_or(Lang::En);

        let mut s = Self {
            screen: Screen::Welcome, lang, sysinfo, services: vec![],
            available_langs,
            selected: 0, overlay_stack: vec![],
            should_quit: false, help_visible: false, welcome_focus: 0, form_queue: None,
            ctrl_hint: false, projects, selected_project: 0,
            hosts: vec![], selected_host: 0, svc_handles: vec![],
            dash_focus: DashFocus::Sidebar,
            sidebar_items: vec![], sidebar_cursor: 0,
            last_refresh: Instant::now(),
            last_podman_statuses: HashMap::new(),
            deploy_rx: None,
            reconcile_rx: None,
            store_rx: None,
            lang_download_rx: None,
            settings,
            store_entries: Vec::new(),
            settings_cursor: 0,
            settings_section: SettingsSection::default(),
            settings_focus: SettingsFocus::default(),
            settings_sidebar_cursor: 0,
            lang_cursor: 0,
            store_langs: Vec::new(),
            store_langs_rx: None,
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

    /// Cycle through En → all loaded languages → En → ... and persist the choice.
    pub fn cycle_lang(&mut self) {
        let langs: Vec<Lang> = std::iter::once(Lang::En)
            .chain(self.available_langs.iter().map(|d| Lang::Dynamic(d)))
            .collect();
        let current = langs.iter().position(|l| *l == self.lang).unwrap_or(0);
        self.lang = langs[(current + 1) % langs.len()];
        self.settings.preferred_lang = match self.lang {
            Lang::En         => None,
            Lang::Dynamic(d) => Some(d.code.to_string()),
        };
        let _ = self.settings.save();
    }

    /// Languages available in the Store that are not yet installed locally.
    ///
    /// Single source of truth for the "downloadable" list — used by the render
    /// layer, keyboard handler, and mouse handler to compute cursor bounds.
    pub fn downloadable_store_langs(&self) -> Vec<&crate::StoreLangEntry> {
        self.store_langs.iter()
            .filter(|e| !self.available_langs.iter().any(|d| d.code == e.code))
            .collect()
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

    // ── Form queue helpers ─────────────────────────────────────────────────

    /// Active form (read-only), regardless of how it was opened.
    pub fn active_form(&self) -> Option<&crate::resource_form::ResourceForm> {
        self.form_queue.as_ref().map(|q| q.active_form())
    }

    /// Active form (mutable), regardless of how it was opened.
    pub fn active_form_mut(&mut self) -> Option<&mut crate::resource_form::ResourceForm> {
        self.form_queue.as_mut().map(|q| q.active_form_mut())
    }

    /// Open a single form (the common case — menu action, edit from sidebar).
    pub fn open_form(&mut self, form: crate::resource_form::ResourceForm) {
        self.form_queue = Some(crate::form_queue::FormQueue::single(form));
        self.screen = Screen::NewProject;
    }

    /// Close the entire form queue and return to Dashboard (or Welcome if no projects).
    pub fn close_form_queue(&mut self) {
        self.form_queue = None;
        self.screen = if self.projects.is_empty() { Screen::Welcome } else { Screen::Dashboard };
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

    /// Reload languages from the i18n directory (e.g. after download).
    pub fn reload_langs(&mut self) {
        let i18n_dir = {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            std::path::PathBuf::from(home).join(".local/share/fsn/i18n")
        };
        self.available_langs = DynamicLang::load_dir(&i18n_dir);
        // Keep current language if still available, else fall back to En.
        let current_code = self.lang.code();
        self.lang = self.available_langs.iter()
            .find(|d| d.code == current_code)
            .map(|d| Lang::Dynamic(d))
            .unwrap_or(Lang::En);
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
                    else { ds.log.push("✓ Done!".into()); }
                    ds.done    = true;
                    ds.success = success;
                    self.deploy_rx = None;
                }
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Detect the user's preferred language from Linux locale environment variables.
///
/// Checks LANGUAGE, LANG, LC_ALL, LC_MESSAGES in that order.
/// Extracts the two-letter language code from values like "de_DE.UTF-8".
/// Returns `None` for "C", "POSIX", or missing/unset values.
fn system_lang_code() -> Option<String> {
    for var in &["LANGUAGE", "LANG", "LC_ALL", "LC_MESSAGES"] {
        if let Ok(val) = std::env::var(var) {
            // LANGUAGE can be colon-separated list; take the first entry.
            let first = val.split(':').next().unwrap_or("");
            // Split on locale separators and take the base language code.
            let code = first.split(['_', '.', '@']).next().unwrap_or("");
            if code.len() >= 2 && !matches!(code, "C" | "POSIX") {
                return Some(code.to_lowercase());
            }
        }
    }
    None
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
        use fsn_core::health::HealthLevel;
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
        use fsn_core::health::HealthLevel;
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
        use fsn_core::health::HealthLevel;
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
        use fsn_core::health::HealthLevel;
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
        use fsn_core::health::HealthLevel;
        let mut state = empty_state();
        state.sidebar_items = vec![
            SidebarItem::Project { slug: "p".into(), name: "MyApp".into(), health: HealthLevel::Ok },
        ];
        state.sidebar_filter = Some("myapp".into());
        assert_eq!(state.visible_sidebar_items().len(), 1);
    }

    #[test]
    fn visible_sidebar_items_filter_no_match() {
        use fsn_core::health::HealthLevel;
        let mut state = empty_state();
        state.sidebar_items = vec![
            SidebarItem::Project { slug: "p".into(), name: "Alpha".into(), health: HealthLevel::Ok },
        ];
        state.sidebar_filter = Some("zzz".into());
        assert!(state.visible_sidebar_items().is_empty());
    }
}
