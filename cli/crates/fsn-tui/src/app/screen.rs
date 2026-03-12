// Screen and dashboard focus enums.
//
// Pattern: State Machine discriminant — Screen is the top-level state that
// drives which renderer and event handler are active. DashFocus is a
// sub-state within Screen::Dashboard.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Dashboard,
    /// Form screen — shows the active form from `form_queue`.
    /// Queue tab bar is visible when `form_queue.has_multiple()`.
    NewProject,
    /// Application settings — store management, preferences.
    Settings,
    /// Store browser — browse and install modules from configured stores.
    Store,
}

// ── Dashboard focus ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashFocus {
    Sidebar,
    Services,
}

// ── Settings ──────────────────────────────────────────────────────────────────

/// Which side of the Settings screen has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsFocus {
    /// Left sidebar — navigating the section list.
    #[default]
    Sidebar,
    /// Right content panel — navigating items within a section.
    Content,
}

/// Active section within the Settings screen.
///
/// Displayed as a sidebar on the left. Each section renders its own
/// content panel on the right.
///
/// Adding a new section:
///   1. Add a variant here.
///   2. Add a `label_key()` arm.
///   3. Add a render function in `ui/settings_screen.rs`.
///   4. Add a key handler in `events.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsSection {
    #[default]
    General,
    Store,
    Languages,
    About,
}

impl SettingsSection {
    pub const ALL: &'static [SettingsSection] = &[
        Self::General,
        Self::Store,
        Self::Languages,
        Self::About,
    ];

    pub fn from_idx(idx: usize) -> Self {
        Self::ALL.get(idx).copied().unwrap_or_default()
    }

    pub fn idx(self) -> usize {
        Self::ALL.iter().position(|&s| s == self).unwrap_or(0)
    }

    /// i18n key for the sidebar label.
    pub fn label_key(self) -> &'static str {
        match self {
            Self::General   => "settings.section.general",
            Self::Store     => "settings.section.store",
            Self::Languages => "settings.section.languages",
            Self::About     => "settings.section.about",
        }
    }
}

// ── Legacy alias (keeps old code compiling during migration) ──────────────────

/// Kept for backward compatibility — maps to SettingsSection.
pub type SettingsTab = SettingsSection;

// ── Store screen focus ────────────────────────────────────────────────────────

/// Which part of the Settings → Store section has focus.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StoreSettingsFocus {
    #[default]
    Repos,
    Modules,
}

/// Which panel of the Store screen has focus.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StoreScreenFocus {
    #[default]
    Sidebar,
    Detail,
}

/// What the Store screen sidebar is showing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StoreSidebarMode {
    #[default]
    ByType,   // grouped by ServiceType category
    All,      // flat list
}
