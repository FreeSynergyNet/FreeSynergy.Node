// Form submit handlers.
//
// Design Pattern: Single Dispatch — handle_form_submit() validates, then
// dispatches to the resource-specific submit_* function.
//
// All submit_* functions read from state.form_queue.active_form() instead
// of the old state.current_form.  On success they call
// state.form_queue.mark_done_and_advance() to close the tab and advance
// to the next pending one (or close the queue entirely if all done).

use std::path::Path;

use anyhow::Result;

use crate::app::{AppState, DashFocus, NotifKind};
use crate::resource_form::{FormErrorKind, ResourceKind};

// ── Generic form submit (validation + dispatch) ───────────────────────────────

pub fn handle_form_submit(state: &mut AppState, root: &Path) -> Result<()> {
    let (missing_t, is_last, kind) = {
        let Some(form) = state.active_form() else { return Ok(()); };
        (form.tab_missing_count(form.active_tab), form.is_last_tab(), form.kind)
    };

    if missing_t > 0 {
        let msg = format!(
            "{} {}",
            missing_t,
            if missing_t == 1 { "required field missing" } else { "required fields missing" }
        );
        if let Some(f) = state.active_form_mut() { f.error = Some(msg); }
        return Ok(());
    }

    if !is_last {
        if let Some(f) = state.active_form_mut() { f.error = None; f.next_tab(); }
        return Ok(());
    }

    let missing = state.active_form().map(|f| f.missing_required().len()).unwrap_or(0);
    if missing > 0 {
        let msg = format!("{} required fields on other tabs are missing", missing);
        if let Some(f) = state.active_form_mut() { f.error = Some(msg); }
        return Ok(());
    }

    kind.dispatch_submit(state, root)?;
    Ok(())
}

// ── ResourceKind::dispatch_submit — OOP dispatch ──────────────────────────────
//
// Design Pattern: OOP — the type knows which submit function to call.
// Placed here (not in resource_form.rs) to avoid circular imports: submit.rs
// already imports ResourceKind; resource_form.rs does not import submit functions.

impl ResourceKind {
    fn dispatch_submit(self, state: &mut AppState, root: &Path) -> Result<()> {
        match self {
            ResourceKind::Project => submit_project(state, root),
            ResourceKind::Service => submit_service(state, root),
            ResourceKind::Host    => submit_host(state, root),
            ResourceKind::Bot     => submit_bot(state, root),
            ResourceKind::Store   => submit_store(state),
        }
    }
}

// ── Shared error helper ───────────────────────────────────────────────────────

/// Write an I/O or system error onto the active form.
///
/// Called at the end of every `submit_*` function's `Some(Err(e))` branch so
/// the 4-line boilerplate is not repeated four times.
fn apply_submit_error(state: &mut AppState, e: anyhow::Error) {
    if let Some(f) = state.active_form_mut() {
        f.error      = Some(format!("{e}"));
        f.error_kind = FormErrorKind::IoError;
    }
}

// ── Queue advancement helper ──────────────────────────────────────────────────

/// Mark the active tab done, advance to next, or close the queue.
/// Called at the end of every successful submit_* function.
fn advance_or_close(state: &mut AppState) {
    let more = state.form_queue
        .as_mut()
        .map(|q| q.mark_done_and_advance())
        .unwrap_or(false);
    if !more {
        state.close_form_queue();
    }
}

// ── Resource-specific submit functions ────────────────────────────────────────

pub fn submit_project(state: &mut AppState, root: &Path) -> Result<()> {
    let result = state.active_form()
        .map(|form| crate::project_form::submit_project_form(form, root));

    match result {
        Some(Ok(())) => {
            // Compute name, slug, and queued tasks from the form in one pass
            // before any state mutation — avoids computing slug twice.
            use crate::project_form::{parse_slot_value, SlotValue};
            let (name, slug, queued_tasks) = {
                let form = state.active_form().unwrap();
                let name = form.field_value("name");
                let slug = form.edit_id.clone()
                    .unwrap_or_else(|| crate::resource_form::slugify(&name));
                let slot_keys = ["iam", "wiki", "mail", "monitoring", "git"];
                let tasks = slot_keys.iter()
                    .filter_map(|&k| {
                        let val = form.field_value(k);
                        match parse_slot_value(&val) {
                            SlotValue::New { class } | SlotValue::Store { class } => {
                                Some(crate::task_queue::TaskKind::NewService {
                                    class:       class.to_string(),
                                    for_project: slug.clone(),
                                })
                            }
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>();
                (name, slug, tasks)
            };

            let (projects, proj_errs) = crate::load_projects(root);
            state.projects = projects;
            for msg in proj_errs { state.push_notif(crate::app::NotifKind::Info, msg); }
            state.selected_project = state.projects.iter()
                .position(|p| p.slug == slug).unwrap_or(0);
            state.rebuild_services();
            state.rebuild_sidebar();
            state.dash_focus = DashFocus::Sidebar;

            // Append service forms to the queue before advancing.
            for kind in queued_tasks {
                let form = kind.build_form(state);
                if let Some(queue) = state.form_queue.as_mut() {
                    queue.push(form, Some(kind));
                }
            }

            advance_or_close(state);
            state.push_notif(NotifKind::Success, format!("Project '{}' saved", name));
        }
        Some(Err(e)) => apply_submit_error(state, e),
        None => {}
    }
    Ok(())
}

pub fn submit_service(state: &mut AppState, root: &Path) -> Result<()> {
    // Read project slug from the form field (user selects it in the form).
    let proj_slug = state.active_form()
        .map(|f| f.field_value("project"))
        .unwrap_or_default();
    if proj_slug.is_empty() {
        if let Some(f) = state.active_form_mut() { f.error = Some("No project selected".into()); }
        return Ok(());
    }
    let proj = state.projects.iter().find(|p| p.slug == proj_slug).cloned();
    let Some(proj) = proj else {
        if let Some(f) = state.active_form_mut() { f.error = Some("Project not found".into()); }
        return Ok(());
    };

    let project_dir  = root.join("projects").join(&proj.slug);
    let services_dir = project_dir.join("services");
    std::fs::create_dir_all(&services_dir)?;

    let result = state.active_form()
        .map(|form| crate::service_form::submit_service_form(form, &services_dir));

    match result {
        Some(Ok(())) => {
            if let Some(form) = state.active_form() {
                let svc_name  = form.field_value("name");
                let svc_class = form.field_value("class");
                let slug      = crate::resource_form::slugify(&svc_name);
                // proj.toml_path is from the pre-reload clone — safe to use here.
                let mut proj_content = std::fs::read_to_string(&proj.toml_path)?;
                if !proj_content.contains(&format!("[load.services.{}]", slug)) {
                    let version = form.field_value("version");
                    let ver     = if version.is_empty() { "latest".to_string() } else { version };
                    let svc_env = form.field_value("env");

                    proj_content.push_str(&format!(
                        "\n[load.services.{slug}]\nservice_class = \"{svc_class}\"\nversion       = \"{ver}\"\n"
                    ));

                    let env_pairs: Vec<String> = svc_env.lines()
                        .filter_map(|line| {
                            let line = line.trim();
                            if line.starts_with('#') || line.is_empty() { return None; }
                            let (k, v) = line.split_once('=')?;
                            let k = k.trim();
                            if k.is_empty() { return None; }
                            let escaped = crate::ui::widgets::toml_escape_str(v.trim());
                            Some(format!("{k} = \"{escaped}\""))
                        })
                        .collect();
                    if !env_pairs.is_empty() {
                        proj_content.push_str(&format!(
                            "\n[load.services.{slug}.env]\n{}\n",
                            env_pairs.join("\n")
                        ));
                    }

                    std::fs::write(&proj.toml_path, proj_content)?;
                }
            }
            let svc_name = state.active_form().map(|f| f.field_value("name")).unwrap_or_default();
            let (projects, proj_errs) = crate::load_projects(root);
            state.projects = projects;
            for msg in proj_errs { state.push_notif(crate::app::NotifKind::Info, msg); }
            // Re-resolve selected_project by slug — filesystem order is non-deterministic
            // and the index may shift after a reload.  proj.slug comes from the pre-reload
            // clone captured at the top of submit_service.
            if let Some(idx) = state.projects.iter().position(|p| p.slug == proj.slug) {
                state.selected_project = idx;
            }
            state.rebuild_services();
            state.rebuild_sidebar();
            state.dash_focus = DashFocus::Services;
            advance_or_close(state);
            state.push_notif(NotifKind::Success, format!("Service '{}' saved", svc_name));
        }
        Some(Err(e)) => apply_submit_error(state, e),
        None => {}
    }
    Ok(())
}

pub fn submit_host(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project) else {
        if let Some(f) = state.active_form_mut() { f.error = Some("No project selected".into()); }
        return Ok(());
    };
    let project_dir = root.join("projects").join(&proj.slug);

    let result = state.active_form()
        .map(|form| crate::host_form::submit_host_form(form, &project_dir));

    match result {
        Some(Ok(())) => {
            let name = state.active_form().map(|f| f.field_value("name")).unwrap_or_default();
            // Reload ALL hosts from ALL projects after save (also calls rebuild_sidebar).
            crate::actions::reload_hosts(state, root);
            state.dash_focus = DashFocus::Sidebar;
            advance_or_close(state);
            state.push_notif(NotifKind::Success, format!("Host '{}' saved", name));
        }
        Some(Err(e)) => apply_submit_error(state, e),
        None => {}
    }
    Ok(())
}

pub fn submit_store(state: &mut AppState) -> Result<()> {
    // Collect all form values first (immutable borrow ends before we mutate state).
    let values = state.active_form().map(|form| {
        let idx        = form.edit_id.as_ref().and_then(|s| s.parse::<usize>().ok());
        let name       = form.field_value("name");
        let url        = form.field_value("url");
        let git_url    = { let v = form.field_value("git_url");    if v.is_empty() { None } else { Some(v) } };
        let local_path = { let v = form.field_value("local_path"); if v.is_empty() { None } else { Some(v) } };
        let enabled    = form.field_bool("enabled");
        (idx, name, url, git_url, local_path, enabled)
    });

    let result: Option<anyhow::Result<String>> = values.map(|(idx, name, url, git_url, local_path, enabled)| {
        let idx = idx.ok_or_else(|| anyhow::anyhow!("invalid store index"))?;
        let store = state.settings.stores.get_mut(idx)
            .ok_or_else(|| anyhow::anyhow!("store index out of range"))?;
        store.name       = name.clone();
        store.url        = url;
        store.git_url    = git_url;
        store.local_path = local_path;
        store.enabled    = enabled;
        state.settings.save().map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(name)
    });

    match result {
        Some(Ok(name)) => {
            advance_or_close(state);
            state.screen = crate::app::Screen::Settings;
            state.push_notif(NotifKind::Success, format!("Store '{}' saved", name));
            // Re-fetch store index with updated settings so the module list reflects changes.
            trigger_store_refetch(state);
        }
        Some(Err(e)) => apply_submit_error(state, e),
        None => {}
    }
    Ok(())
}

/// Trigger a fresh background store fetch and clear stale entries.
///
/// Called after any settings change that affects which stores are enabled or
/// what their URLs are. Clears `store_entries` immediately so the UI shows a
/// loading state rather than stale data while the fetch is in progress.
pub(crate) fn trigger_store_refetch(state: &mut AppState) {
    state.store_entries.clear();
    if state.settings.stores.iter().any(|s| s.enabled) {
        state.store_rx = Some(crate::spawn_store_fetcher(state.settings.clone()));
    } else {
        state.store_rx = None;
    }
}

pub fn submit_bot(state: &mut AppState, root: &Path) -> Result<()> {
    // Read project slug from the form field (user selects it in the form).
    let proj_slug = state.active_form()
        .map(|f| f.field_value("project"))
        .unwrap_or_default();
    if proj_slug.is_empty() {
        if let Some(f) = state.active_form_mut() { f.error = Some("No project selected".into()); }
        return Ok(());
    }

    let result = state.active_form()
        .map(|form| crate::bot_form::submit_bot_form(form, root));

    match result {
        Some(Ok(())) => {
            let name = state.active_form().map(|f| f.field_value("name")).unwrap_or_default();
            state.dash_focus = DashFocus::Services;
            advance_or_close(state);
            state.push_notif(NotifKind::Success, format!("Bot '{}' saved", name));
        }
        Some(Err(e)) => apply_submit_error(state, e),
        None => {}
    }
    Ok(())
}
