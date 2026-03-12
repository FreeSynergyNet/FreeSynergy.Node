// Dependency resolution for FormQueue tabs.
//
// Design Pattern: Strategy — DependencyResolver is a trait so plugin metadata
// can implement it later (from TOML) without changing any queue logic.
//
// TaskKind knows how to:
//   - Build its ResourceForm (build_form)
//   - Report required dependencies (DependencyResolver)
//   - Check if a dependency is already fulfilled (dep_fulfilled)
//   - Convert an unfulfilled dependency into a new TaskKind (dep_to_task)
//
// WorkTask / TaskState / TaskQueue have been replaced by FormQueue (form_queue.rs).

use crate::app::{AppState, ResourceForm, ResourceKind};

// ── Dependency types ──────────────────────────────────────────────────────────

/// The abstract type of a service dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    Proxy,
    IAM,
    Database,
    Mail,
    Git,
    Host,
    ObjectStorage,
}

/// Knows what dependencies this resource type requires.
///
/// Implementable by plugin metadata loaded from TOML — no callers change.
pub trait DependencyResolver {
    fn required_deps(&self) -> &[DependencyKind];
    fn optional_deps(&self) -> &[DependencyKind];
}

// ── Task kind ─────────────────────────────────────────────────────────────────

/// Describes the kind of resource a queued form creates.
/// Carries context (project/host slug) so build_form can pre-fill dropdowns.
#[derive(Debug, Clone, PartialEq)]
pub enum TaskKind {
    NewProject,
    NewHost    { for_project: String },
    NewProxy   { for_host:    String },
    NewIAM     { for_project: String },
    NewMail    { for_project: String },
    NewService { class: String, for_project: String },
}

impl TaskKind {
    /// i18n key for the label shown in the queue tab bar.
    pub fn label_key(&self) -> &'static str {
        match self {
            Self::NewProject        => "task.new_project",
            Self::NewHost { .. }    => "task.new_host",
            Self::NewProxy { .. }   => "task.new_proxy",
            Self::NewIAM { .. }     => "task.new_iam",
            Self::NewMail { .. }    => "task.new_mail",
            Self::NewService { .. } => "task.new_service",
        }
    }

    pub fn resource_kind(&self) -> ResourceKind {
        match self {
            Self::NewProject     => ResourceKind::Project,
            Self::NewHost { .. } => ResourceKind::Host,
            _                    => ResourceKind::Service,
        }
    }

    /// Build the `ResourceForm` for this task, pre-filled as needed.
    pub fn build_form(&self, state: &AppState) -> ResourceForm {
        let p_slugs = state.projects.iter().map(|p| p.slug.clone()).collect::<Vec<_>>();
        let h_slugs = state.hosts.iter().map(|h| h.slug.clone()).collect::<Vec<_>>();
        let cur_p   = state.projects.get(state.selected_project)
            .map(|p| p.slug.as_str()).unwrap_or("");
        let cur_h   = state.hosts.get(state.selected_host)
            .map(|h| h.slug.as_str()).unwrap_or("");

        match self {
            Self::NewProject => crate::project_form::new_project_form(&state.svc_handles, &state.store_entries, &state.available_langs),

            Self::NewHost { .. } => {
                crate::host_form::new_host_form(p_slugs, cur_p)
            }

            Self::NewProxy { .. } => {
                let opts = state.class_options_for_type("proxy", "proxy/zentinel");
                let env  = env_defaults_for("proxy/zentinel");
                crate::service_form::new_service_form_with_class_options(opts, "proxy/zentinel", env.as_deref(), p_slugs, h_slugs, cur_p, cur_h)
            }
            Self::NewIAM { .. } => {
                let opts = state.class_options_for_type("iam", "iam/kanidm");
                let env  = env_defaults_for("iam/kanidm");
                crate::service_form::new_service_form_with_class_options(opts, "iam/kanidm", env.as_deref(), p_slugs, h_slugs, cur_p, cur_h)
            }
            Self::NewMail { .. } => {
                let opts = state.class_options_for_type("mail", "mail/stalwart");
                let env  = env_defaults_for("mail/stalwart");
                crate::service_form::new_service_form_with_class_options(opts, "mail/stalwart", env.as_deref(), p_slugs, h_slugs, cur_p, cur_h)
            }
            Self::NewService { class, .. } => {
                let env = env_defaults_for(class);
                crate::service_form::new_service_form_with_default_class(class, env.as_deref(), p_slugs, h_slugs, cur_p, cur_h)
            }
        }
    }

    /// Check whether a given dependency is already fulfilled in the current app state.
    pub fn dep_fulfilled(dep: DependencyKind, state: &AppState) -> bool {
        match dep {
            DependencyKind::Proxy => state.projects.iter().any(|p| {
                p.config.load.services.values().any(|s| s.service_class.starts_with("proxy/"))
            }),
            DependencyKind::IAM => state.projects.iter().any(|p| {
                p.config.load.services.values().any(|s| s.service_class.starts_with("iam/"))
            }),
            DependencyKind::Mail => state.projects.iter().any(|p| {
                p.config.load.services.values().any(|s| s.service_class.starts_with("mail/"))
            }),
            DependencyKind::Host => !state.hosts.is_empty(),
            _ => false,
        }
    }

    /// Convert an unfulfilled dependency into a new `TaskKind` to enqueue.
    pub fn dep_to_task(&self, dep: DependencyKind) -> Option<TaskKind> {
        match dep {
            DependencyKind::Proxy => Some(TaskKind::NewProxy {
                for_host: self.host_context().unwrap_or_default().to_string(),
            }),
            DependencyKind::IAM => Some(TaskKind::NewIAM {
                for_project: self.project_context().unwrap_or_default().to_string(),
            }),
            DependencyKind::Mail => Some(TaskKind::NewMail {
                for_project: self.project_context().unwrap_or_default().to_string(),
            }),
            DependencyKind::Host => Some(TaskKind::NewHost {
                for_project: self.project_context().unwrap_or_default().to_string(),
            }),
            _ => None,
        }
    }

    fn project_context(&self) -> Option<&str> {
        match self {
            Self::NewHost    { for_project } |
            Self::NewIAM     { for_project } |
            Self::NewMail    { for_project } |
            Self::NewService { for_project, .. } => Some(for_project),
            _ => None,
        }
    }

    fn host_context(&self) -> Option<&str> {
        match self { Self::NewProxy { for_host } => Some(for_host), _ => None }
    }

    /// True if `other` is the same variant (ignoring payload). Avoids duplicates in queue.
    pub fn same_variant(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl DependencyResolver for TaskKind {
    fn required_deps(&self) -> &[DependencyKind] {
        match self {
            Self::NewProject     => &[DependencyKind::IAM, DependencyKind::Host],
            Self::NewHost { .. } => &[DependencyKind::Proxy],
            _                    => &[],
        }
    }
    fn optional_deps(&self) -> &[DependencyKind] { &[] }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Load plugin env defaults for `class_key` from the configured plugins dir.
/// Returns `None` when no Store is configured or the class has no env block.
fn env_defaults_for(class_key: &str) -> Option<String> {
    let dir = fsn_core::config::resolve_plugins_dir_no_fallback()?;
    let s = crate::service_form::load_class_env_defaults(class_key, &dir);
    if s.is_empty() { None } else { Some(s) }
}
