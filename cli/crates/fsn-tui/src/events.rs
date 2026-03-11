// Keyboard event handling — screen router.
//
// Design Pattern: Chain of Responsibility — key events bubble through:
//   1. Global shortcuts (Ctrl+C, F1, Esc-for-help, L-lang-toggle) — always active
//   2. Topmost overlay — captures all input while open
//   3. Active screen handler — delegates to screen-specific module
//
// Dashboard handling lives in events_dashboard.rs.
// Heavy logic is delegated to focused modules:
//   - events_dashboard.rs — dashboard sidebar + services + confirm actions
//   - submit.rs           — form validation and config persistence
//   - actions.rs          — CRUD operations (delete, stop, reload)
//   - deploy_thread.rs    — background deploy/export thread
//   - mouse.rs            — mouse click / scroll / context menu

use std::path::Path;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{AppState, ConfirmAction, OverlayKind, OverlayLayer, Screen};
use crate::resource_form::FormErrorKind;
use crate::ui::form_node::FormAction;
use crate::events_dashboard::{self, handle_new_resource_overlay};
use crate::submit::handle_form_submit;

pub fn handle(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    state.ctrl_hint = key.modifiers.contains(KeyModifiers::CONTROL);

    // Global shortcuts that work on all screens and with overlays open.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        state.should_quit = true;
        return Ok(());
    }
    if key.code == KeyCode::F(1) {
        state.help_visible = !state.help_visible;
        return Ok(());
    }
    // Help sidebar Esc has priority over screen-specific Esc.
    if key.code == KeyCode::Esc && state.help_visible {
        state.help_visible = false;
        return Ok(());
    }

    // Global language toggle — Single Source of Truth for 'L' on non-form screens.
    //
    // Only uppercase L (Shift+L) is global: lowercase 'l' conflicts with
    // per-screen shortcuts (e.g. 'l' = logs in the services panel).
    // Forms handle L per-node via FormAction::LangToggle.
    // Sidebar filter must receive all characters — skip while filter is active.
    if key.code == KeyCode::Char('L')
        && state.form_queue.is_none()
        && state.sidebar_filter.is_none()
    {
        state.cycle_lang();
        return Ok(());
    }

    // Topmost overlay layer captures all input (Ebene system).
    if state.has_overlay() {
        return handle_overlay(key, state, root);
    }

    match state.screen {
        Screen::Welcome    => handle_welcome(key, state),
        Screen::Dashboard  => events_dashboard::handle_dashboard(key, state, root),
        Screen::NewProject => handle_resource_form(key, state, root),
        Screen::Settings   => handle_settings(key, state),
    }
}

// ── Overlay layer handler ─────────────────────────────────────────────────────

fn handle_overlay(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let overlay_kind = state.top_overlay().map(|o| o.kind());

    match overlay_kind {
        Some(OverlayKind::Logs) => {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => { state.pop_overlay(); }
                KeyCode::Up => {
                    if let Some(logs) = state.logs_overlay_mut() {
                        if logs.scroll > 0 { logs.scroll -= 1; }
                    }
                }
                KeyCode::Down => {
                    if let Some(logs) = state.logs_overlay_mut() {
                        let max = logs.lines.len().saturating_sub(1);
                        if logs.scroll < max { logs.scroll += 1; }
                    }
                }
                _ => {}
            }
        }
        Some(OverlayKind::Confirm) => {
            let (data, yes_action) = {
                let (_, d, a) = state.confirm_overlay().unwrap();
                (d.map(|s| s.to_string()), a)
            };
            match key.code {
                KeyCode::Char('j') | KeyCode::Char('J')
                | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    state.pop_overlay();
                    yes_action.execute(state, root, data)?;
                }
                _ => { state.pop_overlay(); }
            }
        }
        Some(OverlayKind::Deploy) => {
            let done = state.top_overlay()
                .map(|o| if let OverlayLayer::Deploy(ref d) = o { d.done } else { false })
                .unwrap_or(false);
            if done && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                state.pop_overlay();
                state.deploy_rx = None;
            }
        }
        Some(OverlayKind::NewResource) => {
            handle_new_resource_overlay(key, state, root)?;
        }
        Some(OverlayKind::ContextMenu) => {
            crate::mouse::handle_context_menu_key(key, state, root)?;
        }
        None => { state.pop_overlay(); }
    }
    Ok(())
}

// ── Welcome screen ────────────────────────────────────────────────────────────

fn handle_welcome(key: KeyEvent, state: &mut AppState) -> Result<()> {
    match key.code {
        KeyCode::Char('q') => state.should_quit = true,
        KeyCode::Char('l') => state.cycle_lang(),
        KeyCode::Left | KeyCode::Right => state.welcome_focus = 1 - state.welcome_focus,
        KeyCode::Enter => {
            if state.welcome_focus == 0 {
                let form = crate::project_form::new_project_form(&state.svc_handles, &state.store_entries);
                state.open_form(form);
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Generic resource form handler ─────────────────────────────────────────────

fn handle_resource_form(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let action = if let Some(f) = state.active_form_mut() {
        f.handle_key(key)
    } else {
        FormAction::Unhandled
    };

    match action {
        FormAction::Cancel => {
            let kind  = state.active_form().map(|f| f.kind);
            let dirty = state.active_form().map(|f| f.is_dirty()).unwrap_or(false);
            if dirty {
                state.push_overlay(OverlayLayer::Confirm {
                    message:    "form.confirm.leave".into(),
                    data:       None,
                    yes_action: ConfirmAction::LeaveForm,
                });
            } else {
                state.close_form_queue();
                if kind == Some(crate::resource_form::ResourceKind::Store) {
                    state.screen = Screen::Settings;
                }
            }
        }
        FormAction::LangToggle => state.cycle_lang(),
        FormAction::Submit     => handle_form_submit(state, root)?,
        FormAction::Consumed   => {}
        FormAction::Unhandled  => {
            if let KeyCode::Char('l') | KeyCode::Char('L') = key.code {
                state.cycle_lang();
            }
        }
        FormAction::AcceptAndNext
        | FormAction::FocusNext | FormAction::FocusPrev
        | FormAction::TabNext  | FormAction::TabPrev
        | FormAction::ValueChanged => {
            if let Some(f) = state.active_form_mut() {
                f.touched = true;
                if f.error_kind == FormErrorKind::Validation {
                    f.error = None;
                }
            }
        }
        FormAction::Quit => state.should_quit = true,
    }
    Ok(())
}

// ── Settings screen ───────────────────────────────────────────────────────────

fn handle_settings(key: KeyEvent, state: &mut AppState) -> Result<()> {
    use crate::app::SettingsTab;

    // Tab key always switches between sections.
    if key.code == KeyCode::Tab {
        state.settings_tab = state.settings_tab.next();
        state.settings_cursor = 0;
        state.lang_cursor = 0;
        return Ok(());
    }

    match state.settings_tab {
        SettingsTab::Stores    => handle_settings_stores(key, state),
        SettingsTab::Languages => handle_settings_languages(key, state),
    }
}

fn handle_settings_stores(key: KeyEvent, state: &mut AppState) -> Result<()> {
    use fsn_core::config::StoreConfig;

    let n = state.settings.stores.len();
    match key.code {
        KeyCode::Up   => crate::ui::cursor::up(&mut state.settings_cursor),
        KeyCode::Down => crate::ui::cursor::down(&mut state.settings_cursor, n),
        KeyCode::Enter => {
            // Open a ResourceForm to edit the selected store.
            if let Some(store) = state.settings.stores.get(state.settings_cursor) {
                let form = crate::settings_form::edit_store_form(state.settings_cursor, store);
                state.open_form(form);
            }
        }
        KeyCode::Char(' ') => {
            if let Some(store) = state.settings.stores.get_mut(state.settings_cursor) {
                store.enabled = !store.enabled;
                let _ = state.settings.save();
            }
        }
        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
            if !state.settings.stores.is_empty() {
                state.settings.stores.remove(state.settings_cursor);
                if state.settings_cursor >= state.settings.stores.len()
                    && state.settings_cursor > 0
                {
                    state.settings_cursor -= 1;
                }
                let _ = state.settings.save();
            }
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            state.settings.stores.push(StoreConfig {
                name: "New Store".into(), url: "https://".into(),
                git_url: None, local_path: None, enabled: false,
            });
            state.settings_cursor = state.settings.stores.len().saturating_sub(1);
            let _ = state.settings.save();
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            state.screen = Screen::Dashboard;
        }
        _ => {}
    }
    Ok(())
}

fn handle_settings_languages(key: KeyEvent, state: &mut AppState) -> Result<()> {
    // Cursor layout:
    //   0               → English (built-in)
    //   1..=installed   → available_langs (downloaded)
    //   installed+1..   → downloadable store langs not yet installed
    let installed   = state.available_langs.len();
    let downloadable = crate::KNOWN_STORE_LANGS.iter()
        .filter(|(code, _)| !state.available_langs.iter().any(|d| d.code == *code))
        .count();
    let n_total = 1 + installed + downloadable;

    match key.code {
        KeyCode::Up   => crate::ui::cursor::up(&mut state.lang_cursor),
        KeyCode::Down => crate::ui::cursor::down(&mut state.lang_cursor, n_total),

        KeyCode::Enter => {
            let idx = state.lang_cursor;
            if idx == 0 {
                // Activate English.
                state.lang = crate::app::Lang::En;
                state.settings.preferred_lang = None;
                let _ = state.settings.save();
            } else if idx <= installed {
                // Activate an already-installed language.
                if let Some(dl) = state.available_langs.get(idx - 1) {
                    state.lang = crate::app::Lang::Dynamic(dl);
                    state.settings.preferred_lang = Some(dl.code.to_string());
                    let _ = state.settings.save();
                }
            } else {
                // Download a store language — same as 'f'.
                trigger_lang_download(state, idx - 1 - installed);
            }
        }

        // 'f' / 'F' = fetch (download) the selected store language.
        KeyCode::Char('f') | KeyCode::Char('F') => {
            let idx = state.lang_cursor;
            if idx > installed {
                trigger_lang_download(state, idx - 1 - installed);
            }
        }

        KeyCode::Delete | KeyCode::Char('d') | KeyCode::Char('D') => {
            // Remove a downloaded language file (built-in English cannot be removed).
            if state.lang_cursor > 0 && state.lang_cursor <= installed {
                if let Some(dl) = state.available_langs.get(state.lang_cursor - 1) {
                    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
                    let path = std::path::PathBuf::from(home)
                        .join(".local/share/fsn/i18n")
                        .join(format!("{}.toml", dl.code));
                    let _ = std::fs::remove_file(&path);
                    if matches!(state.lang, crate::app::Lang::Dynamic(d) if d.code == dl.code) {
                        state.lang = crate::app::Lang::En;
                        state.settings.preferred_lang = None;
                        let _ = state.settings.save();
                    }
                    state.reload_langs();
                    state.lang_cursor = state.lang_cursor.min(state.available_langs.len());
                }
            }
        }

        KeyCode::Esc | KeyCode::Char('q') => {
            state.screen = Screen::Dashboard;
        }
        _ => {}
    }
    Ok(())
}

/// Public wrapper — called by mouse.rs on double-click of a downloadable lang row.
pub(crate) fn trigger_lang_download_pub(state: &mut AppState, download_idx: usize) {
    trigger_lang_download(state, download_idx);
}

/// Spawn a background download for the Nth downloadable store language.
fn trigger_lang_download(state: &mut AppState, download_idx: usize) {
    if state.lang_download_rx.is_some() {
        state.push_notif(crate::app::NotifKind::Info, "Download already in progress…");
        return;
    }
    let uninstalled: Vec<&str> = crate::KNOWN_STORE_LANGS.iter()
        .filter(|(code, _)| !state.available_langs.iter().any(|d| d.code == *code))
        .map(|(code, _)| *code)
        .collect();
    if let Some(&code) = uninstalled.get(download_idx) {
        state.push_notif(crate::app::NotifKind::Info, format!("Downloading {}…", code.to_uppercase()));
        state.lang_download_rx = Some(crate::spawn_lang_downloader(code, state.settings.clone()));
    }
}
