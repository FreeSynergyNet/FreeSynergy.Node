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
        }
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
            let name = state.active_form().map(|f| f.field_value("name")).unwrap_or_default();

            // Extract "new:" / "store:" slot values BEFORE advancing the queue.
            let queued_tasks: Vec<crate::task_queue::TaskKind> = {
                let form = state.active_form().unwrap();
                let slug = form.edit_id.clone()
                    .unwrap_or_else(|| crate::resource_form::slugify(&form.field_value("name")));
                let slot_keys = ["iam", "wiki", "mail", "monitoring", "git"];
                slot_keys.iter()
                    .filter_map(|&k| {
                        let val = form.field_value(k);
                        if let Some(class) = val.strip_prefix("new:") {
                            return Some(crate::task_queue::TaskKind::NewService {
                                class:       class.to_string(),
                                for_project: slug.clone(),
                            });
                        }
                        if let Some(class) = val.strip_prefix("store:") {
                            return Some(crate::task_queue::TaskKind::NewService {
                                class:       class.to_string(),
                                for_project: slug.clone(),
                            });
                        }
                        None
                    })
                    .collect()
            };

            state.projects = crate::load_projects(root);
            if let Some(form) = state.active_form() {
                let slug = form.edit_id.clone()
                    .unwrap_or_else(|| crate::resource_form::slugify(&form.field_value("name")));
                state.selected_project = state.projects.iter()
                    .position(|p| p.slug == slug).unwrap_or(0);
            }
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
        Some(Err(e)) => {
            if let Some(f) = state.active_form_mut() {
                f.error = Some(format!("{e}"));
                f.error_kind = FormErrorKind::IoError;
            }
        }
        None => {}
    }
    Ok(())
}

pub fn submit_service(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project).cloned() else {
        if let Some(f) = state.active_form_mut() { f.error = Some("No project selected".into()); }
        return Ok(());
    };

    let project_dir  = root.join("projects").join(&proj.slug);
    let services_dir = project_dir.join("services");
    std::fs::create_dir_all(&services_dir)?;

    let result = state.active_form()
        .map(|form| crate::service_form::submit_service_form(form, &services_dir, &proj.slug));

    match result {
        Some(Ok(())) => {
            if let Some(form) = state.active_form() {
                let svc_name  = form.field_value("name");
                let svc_class = form.field_value("class");
                let slug      = crate::resource_form::slugify(&svc_name);
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
                            let (k, v) = line.split_once('=')?;
                            let k = k.trim();
                            if k.is_empty() { return None; }
                            Some(format!("{k} = \"{}\"", v.trim()))
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
            state.projects = crate::load_projects(root);
            state.rebuild_services();
            state.rebuild_sidebar();
            state.dash_focus = DashFocus::Services;
            advance_or_close(state);
            state.push_notif(NotifKind::Success, format!("Service '{}' saved", svc_name));
        }
        Some(Err(e)) => {
            if let Some(f) = state.active_form_mut() {
                f.error = Some(format!("{e}"));
                f.error_kind = FormErrorKind::IoError;
            }
        }
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
            state.hosts = crate::load_hosts(&project_dir);
            state.rebuild_sidebar();
            state.dash_focus = DashFocus::Sidebar;
            advance_or_close(state);
            state.push_notif(NotifKind::Success, format!("Host '{}' saved", name));
        }
        Some(Err(e)) => {
            if let Some(f) = state.active_form_mut() {
                f.error = Some(format!("{e}"));
                f.error_kind = FormErrorKind::IoError;
            }
        }
        None => {}
    }
    Ok(())
}

pub fn submit_bot(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project).cloned() else {
        if let Some(f) = state.active_form_mut() { f.error = Some("No project selected".into()); }
        return Ok(());
    };
    let project_dir = root.join("projects").join(&proj.slug);

    let result = state.active_form()
        .map(|form| crate::bot_form::submit_bot_form(form, &project_dir, &proj.slug));

    match result {
        Some(Ok(())) => {
            let name = state.active_form().map(|f| f.field_value("name")).unwrap_or_default();
            state.dash_focus = DashFocus::Services;
            advance_or_close(state);
            state.push_notif(NotifKind::Success, format!("Bot '{}' saved", name));
        }
        Some(Err(e)) => {
            if let Some(f) = state.active_form_mut() {
                f.error = Some(format!("{e}"));
                f.error_kind = FormErrorKind::IoError;
            }
        }
        None => {}
    }
    Ok(())
}
