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
use crate::events_dashboard::{self, execute_confirm_action, handle_new_resource_overlay};
use crate::submit::{handle_form_submit, handle_wizard_submit};

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
    // Forms and TaskWizard handle L per-node via FormAction::LangToggle.
    // Sidebar filter must receive all characters — skip while filter is active.
    if key.code == KeyCode::Char('L')
        && state.current_form.is_none()
        && state.sidebar_filter.is_none()
    {
        state.lang = state.lang.toggle();
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
        Screen::TaskWizard => handle_wizard(key, state, root),
        Screen::Settings   => handle_settings(key, state),
    }
}

// ── Overlay layer handler ─────────────────────────────────────────────────────

fn handle_overlay(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    // Read the discriminant first — this ends the immutable borrow so we can
    // mutate state freely inside each arm without borrow-checker conflicts.
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
            // Extract data BEFORE popping — the overlay is gone afterwards.
            let (data, yes_action) = {
                let (_, d, a) = state.confirm_overlay().unwrap();
                (d.map(|s| s.to_string()), a)
            };
            match key.code {
                // Accept: j/J (German "Ja") or y/Y (English "Yes").
                KeyCode::Char('j') | KeyCode::Char('J')
                | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    state.pop_overlay();
                    execute_confirm_action(state, root, data, yes_action)?;
                }
                // Any other key = cancel (close overlay, take no action).
                _ => { state.pop_overlay(); }
            }
        }
        Some(OverlayKind::Deploy) => {
            // Only closeable once the background thread has finished.
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
        // 'l' (lowercase) — uppercase 'L' is handled globally in handle().
        KeyCode::Char('l') => state.lang = state.lang.toggle(),
        // Toggle between the two buttons (New Project / Open Project).
        KeyCode::Left | KeyCode::Right => state.welcome_focus = 1 - state.welcome_focus,
        KeyCode::Enter => {
            if state.welcome_focus == 0 {
                state.current_form = Some(crate::project_form::new_project_form(&state.svc_handles, &state.store_entries));
                state.screen = Screen::NewProject;
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Generic resource form handler ─────────────────────────────────────────────

fn handle_resource_form(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let action = if let Some(ref mut form) = state.current_form {
        form.handle_key(key)
    } else {
        FormAction::Unhandled
    };

    match action {
        FormAction::Cancel => {
            let dirty = state.current_form.as_ref().map(|f| f.is_dirty()).unwrap_or(false);
            if dirty {
                state.push_overlay(OverlayLayer::Confirm {
                    message:    "form.confirm.leave".into(),
                    data:       None,
                    yes_action: ConfirmAction::LeaveForm,
                });
            } else {
                state.current_form = None;
                state.screen = if state.projects.is_empty() { Screen::Welcome } else { Screen::Dashboard };
            }
        }
        FormAction::LangToggle => state.lang = state.lang.toggle(),
        FormAction::Submit     => handle_form_submit(state, root)?,
        FormAction::Consumed   => {}
        FormAction::Unhandled  => {
            if let KeyCode::Char('l') | KeyCode::Char('L') = key.code {
                state.lang = state.lang.toggle();
            }
        }
        // Navigation and value changes — mark form as touched and run live validation.
        FormAction::AcceptAndNext
        | FormAction::FocusNext | FormAction::FocusPrev
        | FormAction::TabNext  | FormAction::TabPrev
        | FormAction::ValueChanged => {
            if let Some(ref mut form) = state.current_form {
                form.touched = true;
                // Clear a previous submit-level validation error so it doesn't
                // persist after the user starts actively editing.
                if form.error_kind == FormErrorKind::Validation {
                    form.error = None;
                }
            }
        }
        FormAction::Quit => state.should_quit = true,
    }
    Ok(())
}

// ── Task wizard ───────────────────────────────────────────────────────────────

fn handle_wizard(key: KeyEvent, state: &mut AppState, root: &Path) -> Result<()> {
    let action = if let Some(ref mut queue) = state.task_queue {
        if let Some(task) = queue.tasks.get_mut(queue.active) {
            if let Some(ref mut form) = task.form { form.handle_key(key) }
            else { FormAction::Unhandled }
        } else { FormAction::Unhandled }
    } else { FormAction::Unhandled };

    match action {
        FormAction::Cancel     => confirm_leave_wizard(state),
        FormAction::LangToggle => state.lang = state.lang.toggle(),
        FormAction::Submit     => handle_wizard_submit(state, root)?,
        FormAction::Consumed   => {}
        FormAction::Unhandled  => {
            match key.code {
                KeyCode::Esc => confirm_leave_wizard(state),
                // 'l' (lowercase) for non-text nodes — 'L' is handled globally.
                KeyCode::Char('l') => state.lang = state.lang.toggle(),
                _ => {}
            }
        }
        FormAction::AcceptAndNext
        | FormAction::FocusNext | FormAction::FocusPrev
        | FormAction::TabNext  | FormAction::TabPrev
        | FormAction::ValueChanged => {}
        FormAction::Quit => state.should_quit = true,
    }
    Ok(())
}

fn confirm_leave_wizard(state: &mut AppState) {
    let dirty = state.task_queue.as_ref()
        .and_then(|q| q.tasks.get(q.active)?.form.as_ref().map(|f| f.is_dirty()))
        .unwrap_or(false);
    if dirty {
        state.push_overlay(OverlayLayer::Confirm {
            message:    "form.confirm.leave".into(),
            data:       None,
            yes_action: ConfirmAction::LeaveWizard,
        });
    } else {
        state.task_queue = None;
        state.screen = Screen::Dashboard;
    }
}

// ── Settings screen ───────────────────────────────────────────────────────────

fn handle_settings(key: KeyEvent, state: &mut AppState) -> Result<()> {
    use fsn_core::config::StoreConfig;

    let n_stores = state.settings.stores.len();
    match key.code {
        KeyCode::Up => {
            if state.settings_cursor > 0 { state.settings_cursor -= 1; }
        }
        KeyCode::Down => {
            if n_stores > 0 && state.settings_cursor < n_stores - 1 {
                state.settings_cursor += 1;
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

