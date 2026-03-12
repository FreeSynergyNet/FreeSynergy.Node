// Overlay layer system — modal screens drawn on top of the main UI.
//
// Pattern: Composite (stack-based) + Discriminant — OverlayLayer is the enum
// of all possible modal layers. OverlayKind is a cheap Copy discriminant that
// allows borrow-safe inspection of the top layer before mutating it.
//
// "Ebene" concept: the topmost overlay captures all input; layers below are
// frozen until the top layer is dismissed.

use crate::app::sidebar::SidebarItem;

// ── Logs overlay ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LogsState {
    pub service_name: String,
    pub lines:        Vec<String>,
    pub scroll:       usize,
}

// ── Deploy overlay ────────────────────────────────────────────────────────────

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

// ── OverlayKind — borrow-safe discriminant ────────────────────────────────────

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

// ── OverlayLayer ──────────────────────────────────────────────────────────────

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

// ── ConfirmAction ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    DeleteProject,
    DeleteService,
    DeleteHost,
    StopService,
    /// Close the form queue (abandon all pending tabs — user confirmed).
    LeaveForm,
    Quit,
    /// Mark the module (data = module ID) as installed in AppSettings.
    MarkModuleInstalled,
    /// Mark the module (data = module ID) as uninstalled in AppSettings.
    MarkModuleUninstalled,
}

// ── ActionSource — context menu origin ───────────────────────────────────────

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

// ── ContextAction — right-click menu actions ─────────────────────────────────
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
