// Form submit handlers.
//
// Handles validation flow and persists resource configs to disk.
// Called by events.rs after form validation passes.

use std::path::Path;

use anyhow::Result;

use crate::app::{AppState, DashFocus, NotifKind, Screen};
use crate::resource_form::{FormErrorKind, ResourceKind};

// ── Generic form submit (validation + dispatch) ───────────────────────────────

pub fn handle_form_submit(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(ref form) = state.current_form else { return Ok(()); };
    let missing_t = form.tab_missing_count(form.active_tab);

    if missing_t > 0 {
        let msg = format!(
            "{} {}",
            missing_t,
            if missing_t == 1 { "required field missing" } else { "required fields missing" }
        );
        if let Some(ref mut f) = state.current_form { f.error = Some(msg); }
        return Ok(());
    }

    if !form.is_last_tab() {
        if let Some(ref mut f) = state.current_form { f.error = None; f.next_tab(); }
        return Ok(());
    }

    let missing = form.missing_required();
    if !missing.is_empty() {
        let msg = format!("{} required fields on other tabs are missing", missing.len());
        if let Some(ref mut f) = state.current_form { f.error = Some(msg); }
        return Ok(());
    }

    let kind = state.current_form.as_ref().map(|f| f.kind);
    match kind {
        Some(ResourceKind::Project) => submit_project(state, root)?,
        Some(ResourceKind::Service) => submit_service(state, root)?,
        Some(ResourceKind::Host)    => submit_host(state, root)?,
        Some(ResourceKind::Bot)     => submit_bot(state, root)?,
        None => {}
    }
    Ok(())
}

// ── Wizard submit (validation + dispatch + queue advance) ─────────────────────

pub fn handle_wizard_submit(state: &mut AppState, root: &Path) -> Result<()> {
    let (missing_tab, is_last, missing_all, kind) = {
        let Some(ref queue) = state.task_queue else { return Ok(()); };
        let Some(task) = queue.tasks.get(queue.active) else { return Ok(()); };
        let Some(ref form) = task.form else { return Ok(()); };
        (
            form.tab_missing_count(form.active_tab),
            form.is_last_tab(),
            form.missing_required().len(),
            task.kind.resource_kind(),
        )
    };

    if missing_tab > 0 {
        let msg = format!(
            "{} {}",
            missing_tab,
            if missing_tab == 1 { "required field missing" } else { "required fields missing" }
        );
        if let Some(ref mut queue) = state.task_queue {
            if let Some(task) = queue.tasks.get_mut(queue.active) {
                if let Some(ref mut form) = task.form { form.error = Some(msg); }
            }
        }
        return Ok(());
    }

    if !is_last {
        if let Some(ref mut queue) = state.task_queue {
            if let Some(task) = queue.tasks.get_mut(queue.active) {
                if let Some(ref mut form) = task.form { form.error = None; form.next_tab(); }
            }
        }
        return Ok(());
    }

    if missing_all > 0 {
        let msg = format!("{} required fields on other tabs are missing", missing_all);
        if let Some(ref mut queue) = state.task_queue {
            if let Some(task) = queue.tasks.get_mut(queue.active) {
                if let Some(ref mut form) = task.form { form.error = Some(msg); }
            }
        }
        return Ok(());
    }

    // The submit_* functions read from state.current_form, not task_queue.
    // Temporarily move the form out of the task and into current_form so the
    // same submit code path works for both the wizard and the standalone form screen.
    let form = if let Some(ref mut queue) = state.task_queue {
        queue.tasks.get_mut(queue.active).and_then(|t| t.form.take())
    } else { None };

    let Some(form) = form else { return Ok(()); };
    state.current_form = Some(form);

    let submit_result = match kind {
        ResourceKind::Project => submit_project(state, root),
        ResourceKind::Host    => submit_host(state, root),
        ResourceKind::Service => submit_service(state, root),
        ResourceKind::Bot     => submit_bot(state, root),
    };

    // If submit failed, put the form back so the user sees the error message.
    // submit_* clears current_form on success, so a None here means success.
    if let Some(ref mut queue) = state.task_queue {
        if let Some(task) = queue.tasks.get_mut(queue.active) {
            if task.form.is_none() {
                task.form = state.current_form.take();
            }
        }
    }

    submit_result?;

    // Submit succeeded — advance the wizard queue
    let more = if let Some(mut queue) = state.task_queue.take() {
        let has_more = queue.on_task_saved(state);
        state.task_queue = Some(queue);
        has_more
    } else {
        false
    };

    if !more {
        state.task_queue = None;
        state.screen = Screen::Dashboard;
    } else {
        state.screen = Screen::TaskWizard;
    }
    state.current_form = None;
    Ok(())
}

// ── Resource-specific submit functions ────────────────────────────────────────

pub fn submit_project(state: &mut AppState, root: &Path) -> Result<()> {
    let result = state.current_form.as_ref()
        .map(|form| crate::project_form::submit_project_form(form, root));

    match result {
        Some(Ok(())) => {
            let name = state.current_form.as_ref().map(|f| f.field_value("name")).unwrap_or_default();

            // Extract slot values BEFORE clearing current_form — check for pending service tasks.
            let queued_tasks: Vec<crate::task_queue::TaskKind> = {
                let form = state.current_form.as_ref().unwrap();
                let slug = form.edit_id.clone()
                    .unwrap_or_else(|| crate::resource_form::slugify(&form.field_value("name")));
                let slot_keys = ["iam", "wiki", "mail", "monitoring", "git"];
                slot_keys.iter()
                    .filter_map(|&k| {
                        let val = form.field_value(k);
                        // Strip "new:" prefix (local available class)
                        if let Some(class) = val.strip_prefix("new:") {
                            return Some(crate::task_queue::TaskKind::NewService {
                                class:       class.to_string(),
                                for_project: slug.clone(),
                            });
                        }
                        // Strip "store:" prefix (store module — treat same as new for now)
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
            if let Some(ref form) = state.current_form {
                let slug = form.edit_id.clone()
                    .unwrap_or_else(|| crate::resource_form::slugify(&form.field_value("name")));
                state.selected_project = state.projects.iter()
                    .position(|p| p.slug == slug).unwrap_or(0);
            }
            state.rebuild_services();
            state.rebuild_sidebar();
            state.dash_focus = DashFocus::Sidebar;
            state.current_form = None;

            // Queue service tasks if any slot had "new:" or "store:" value
            if !queued_tasks.is_empty() {
                use crate::task_queue::{TaskQueue, WorkTask};
                let mut tasks: Vec<WorkTask> = queued_tasks.into_iter()
                    .map(WorkTask::new)
                    .collect();
                tasks[0].activate(state);
                let queue = TaskQueue { tasks, active: 0 };
                state.task_queue = Some(queue);
                state.screen = Screen::TaskWizard;
            } else {
                state.screen = Screen::Dashboard;
            }

            state.push_notif(NotifKind::Success, format!("Project '{}' saved", name));
        }
        Some(Err(e)) => {
            if let Some(ref mut form) = state.current_form { form.error = Some(format!("{e}")); form.error_kind = FormErrorKind::IoError; }
        }
        None => {}
    }
    Ok(())
}

pub fn submit_service(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project).cloned() else {
        if let Some(ref mut f) = state.current_form { f.error = Some("No project selected".into()); }
        return Ok(());
    };

    let project_dir  = root.join("projects").join(&proj.slug);
    let services_dir = project_dir.join("services");
    std::fs::create_dir_all(&services_dir)?;

    let result = state.current_form.as_ref()
        .map(|form| crate::service_form::submit_service_form(form, &services_dir, &proj.slug));

    match result {
        Some(Ok(())) => {
            if let Some(ref form) = state.current_form {
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
            let svc_name = state.current_form.as_ref().map(|f| f.field_value("name")).unwrap_or_default();
            state.projects = crate::load_projects(root);
            state.rebuild_services();
            state.rebuild_sidebar();
            state.screen      = Screen::Dashboard;
            state.dash_focus  = DashFocus::Services;
            state.current_form = None;
            state.push_notif(NotifKind::Success, format!("Service '{}' saved", svc_name));
        }
        Some(Err(e)) => {
            if let Some(ref mut form) = state.current_form { form.error = Some(format!("{e}")); form.error_kind = FormErrorKind::IoError; }
        }
        None => {}
    }
    Ok(())
}

pub fn submit_host(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project) else {
        if let Some(ref mut f) = state.current_form { f.error = Some("No project selected".into()); }
        return Ok(());
    };
    let project_dir = root.join("projects").join(&proj.slug);

    let result = state.current_form.as_ref()
        .map(|form| crate::host_form::submit_host_form(form, &project_dir));

    match result {
        Some(Ok(())) => {
            let name = state.current_form.as_ref().map(|f| f.field_value("name")).unwrap_or_default();
            state.hosts = crate::load_hosts(&project_dir);
            state.rebuild_sidebar();
            state.screen      = Screen::Dashboard;
            state.dash_focus  = DashFocus::Sidebar;
            state.current_form = None;
            state.push_notif(NotifKind::Success, format!("Host '{}' saved", name));
        }
        Some(Err(e)) => {
            if let Some(ref mut form) = state.current_form { form.error = Some(format!("{e}")); form.error_kind = FormErrorKind::IoError; }
        }
        None => {}
    }
    Ok(())
}

pub fn submit_bot(state: &mut AppState, root: &Path) -> Result<()> {
    let Some(proj) = state.projects.get(state.selected_project).cloned() else {
        if let Some(ref mut f) = state.current_form { f.error = Some("No project selected".into()); }
        return Ok(());
    };
    let project_dir = root.join("projects").join(&proj.slug);

    let result = state.current_form.as_ref()
        .map(|form| crate::bot_form::submit_bot_form(form, &project_dir, &proj.slug));

    match result {
        Some(Ok(())) => {
            let name = state.current_form.as_ref().map(|f| f.field_value("name")).unwrap_or_default();
            state.screen      = Screen::Dashboard;
            state.dash_focus  = DashFocus::Services;
            state.current_form = None;
            state.push_notif(NotifKind::Success, format!("Bot '{}' saved", name));
        }
        Some(Err(e)) => {
            if let Some(ref mut form) = state.current_form { form.error = Some(format!("{e}")); form.error_kind = FormErrorKind::IoError; }
        }
        None => {}
    }
    Ok(())
}
