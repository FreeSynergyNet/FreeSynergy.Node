// ClickMap — central registry for all clickable UI elements.
//
// Design Pattern: Registry (variant of Command Pattern)
//
//   Every rendered widget registers itself here during the render pass:
//     cmap.push(rect, ClickTarget::LangToggle)
//     cmap.push(rect, ClickTarget::FormField { ... })
//
//   Mouse dispatch queries the map once — no per-screen if/else chains:
//     match click_map.hit(col, row) { Some(LangToggle) => ..., ... }
//
// Adding a new clickable widget:
//   1. Add a variant to `ClickTarget`.
//   2. Call `cmap.push(rect, YourTarget)` in the widget's render function.
//   3. Add one match arm in `handle_left_click_target()` in mouse.rs.
//
// No other files need to change.  The map is cleared by each screen's render
// function at the start of each frame and rebuilt from scratch.
//
// "Topmost wins" for overlapping rects: last-registered entry is returned
// first, so overlays (registered after fields) naturally win hit-tests.

use ratatui::layout::Rect;

// ── ClickTarget ───────────────────────────────────────────────────────────────

/// What happens when the user left-clicks a registered area.
///
/// Each variant is self-contained: it carries enough context for the
/// dispatcher to act without additional lookups.
#[derive(Debug, Clone)]
pub enum ClickTarget {
    /// Toggle the UI language (DE ↔ EN).
    /// Works on every screen and regardless of overlay state.
    LangToggle,

    /// Submit the current form (same as Ctrl+S).
    FormSubmit,

    /// A focusable form field.
    /// `slot`     — index within the current tab (used to set `active_field`).
    /// `node_idx` — global index into `ResourceForm::nodes`.
    /// `rect`     — rendered Rect, forwarded to `FormNode::handle_mouse`.
    FormField { slot: usize, node_idx: usize, rect: Rect },

    /// A tab in the queue tab bar — switches the active form to `idx`.
    QueueTab { idx: usize },

    /// A navigation tab in the header bar.
    /// `index` maps to: 0=Projects, 1=Hosts, 2=Services, 3=Store, 4=Settings.
    NavTab { index: usize },

    /// A section row in the Settings sidebar.
    /// `idx` = index into `SettingsSection::ALL`.
    SettingsSidebar { idx: usize },

    /// A store row in the Settings → Stores tab.
    /// `idx` = index into `state.settings.stores`.
    SettingsCursor { idx: usize },

    /// A language row in the Settings → Languages tab.
    /// `idx` = absolute cursor index (0 = English, 1+ = installed, then downloadable).
    LangCursor { idx: usize },

    /// A button in the Welcome overlay popup.
    /// `index`: 0 = "New Project", 1 = "Open Project" (disabled).
    WelcomeButton { index: usize },
}

// ── ClickMap ──────────────────────────────────────────────────────────────────

/// Per-frame registry of all clickable UI regions.
///
/// Cleared and rebuilt every render pass. Stored on `AppState` so all render
/// and event modules can access it without additional plumbing.
#[derive(Debug, Default, Clone)]
pub struct ClickMap(Vec<(Rect, ClickTarget)>);

impl ClickMap {
    pub fn new() -> Self { Self::default() }

    /// Remove all entries. Call once at the start of each screen render.
    pub fn clear(&mut self) { self.0.clear(); }

    /// Register a clickable area with its action.
    ///
    /// Zero-sized rects are silently ignored (off-screen / hidden widgets).
    pub fn push(&mut self, rect: Rect, target: ClickTarget) {
        if rect.width > 0 && rect.height > 0 {
            self.0.push((rect, target));
        }
    }

    /// Return the topmost registered target at terminal position (col, row).
    ///
    /// "Topmost" = last registered, because render order means later elements
    /// (overlays, floating menus) are drawn on top and registered last.
    pub fn hit(&self, col: u16, row: u16) -> Option<&ClickTarget> {
        self.0.iter().rev()
            .find(|(r, _)| col >= r.x && col < r.right() && row >= r.y && row < r.bottom())
            .map(|(_, t)| t)
    }
}
