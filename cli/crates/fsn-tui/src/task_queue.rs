// Task Wizard — progressive resource setup with dependency tracking.
//
// A TaskQueue is a linear sequence of WorkTasks. Each task holds a
// ResourceForm that the user fills out and saves independently.
// Saving a task automatically checks for unfulfilled dependencies
// and appends new tasks as needed — the wizard grows as you work.
//
// DependencyResolver is a trait so plugin metadata can implement it
// later without changing any wizard logic.

use crate::app::{AppState, Lang, ResourceForm, ResourceKind};

// ── Dependency types ──────────────────────────────────────────────────────────

/// The abstract type of a service dependency.
/// Does not specify which concrete service fulfills it — that is up to the user.
///
/// Designed for future plugin use: plugin TOML files will declare their
/// dependencies using these variants, then implement `DependencyResolver`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    /// A reverse-proxy on the host (Zentinel, Traefik, …).
    Proxy,
    /// An Identity & Access Management provider (Kanidm, Keycloak, …).
    IAM,
    /// A relational database (Postgres, SQLite, …).
    Database,
    /// An SMTP-capable mail server (Stalwart, …).
    Mail,
    /// A Git hosting service (Forgejo, Gitea, …).
    Git,
    /// S3-compatible object storage (MinIO, …).
    ObjectStorage,
}

/// Knows what dependencies this resource type requires.
///
/// Currently implemented by `TaskKind` (compile-time, Rust code).
/// Later: implementable by `PluginMetadata` loaded from TOML — same interface,
/// different source. No callers need to change when plugins are introduced.
pub trait DependencyResolver {
    /// Must be fulfilled before this resource can run correctly.
    /// Unfulfilled entries automatically spawn new wizard tasks.
    fn required_deps(&self) -> &[DependencyKind];

    /// May be useful but are not required.
    /// Not spawned automatically — user can add them manually.
    fn optional_deps(&self) -> &[DependencyKind];
}

// ── Task kind ─────────────────────────────────────────────────────────────────

/// The kind of resource a `WorkTask` creates.
///
/// Each variant knows how to:
/// - Build its `ResourceForm`
/// - Report its required dependencies
/// - Convert a dependency into a new `TaskKind`
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
    /// i18n key for the label shown in the task tab bar.
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

    /// The `ResourceKind` this task submits — used by the event handler
    /// to dispatch to the correct submit function.
    pub fn resource_kind(&self) -> ResourceKind {
        match self {
            Self::NewProject        => ResourceKind::Project,
            Self::NewHost { .. }    => ResourceKind::Host,
            _                       => ResourceKind::Service,
        }
    }

    /// Build the `ResourceForm` for this task, pre-filled as needed.
    pub fn build_form(&self, state: &AppState) -> ResourceForm {
        match self {
            Self::NewProject => crate::project_form::new_project_form(),

            Self::NewHost { .. } => {
                let slugs   = state.projects.iter().map(|p| p.slug.clone()).collect();
                let current = state.projects.get(state.selected_project)
                    .map(|p| p.slug.as_str()).unwrap_or("");
                crate::host_form::new_host_form(slugs, current)
            }

            Self::NewProxy { .. } =>
                crate::service_form::new_service_form_with_default_class("proxy/zentinel"),
            Self::NewIAM { .. } =>
                crate::service_form::new_service_form_with_default_class("iam/kanidm"),
            Self::NewMail { .. } =>
                crate::service_form::new_service_form_with_default_class("mail/stalwart"),
            Self::NewService { class, .. } =>
                crate::service_form::new_service_form_with_default_class(class),
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
            _ => None,
        }
    }

    fn project_context(&self) -> Option<&str> {
        match self {
            Self::NewHost    { for_project } => Some(for_project),
            Self::NewIAM     { for_project } => Some(for_project),
            Self::NewMail    { for_project } => Some(for_project),
            Self::NewService { for_project, .. } => Some(for_project),
            _ => None,
        }
    }

    fn host_context(&self) -> Option<&str> {
        match self { Self::NewProxy { for_host } => Some(for_host), _ => None }
    }

    /// True if `other` is the same variant (ignoring payload data).
    /// Used to avoid duplicate tasks in the queue.
    pub fn same_variant(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

impl DependencyResolver for TaskKind {
    fn required_deps(&self) -> &[DependencyKind] {
        match self {
            Self::NewProject    => &[DependencyKind::IAM],
            Self::NewHost { .. } => &[DependencyKind::Proxy],
            _                   => &[],
        }
    }

    fn optional_deps(&self) -> &[DependencyKind] { &[] }
}

// ── Task state ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Not yet started — form not yet initialised.
    Pending,
    /// Currently being edited by the user.
    Active,
    /// Saved successfully.
    Done,
}

// ── Work task ─────────────────────────────────────────────────────────────────

/// One step in the wizard: a kind, its run state, and its form.
pub struct WorkTask {
    pub kind:  TaskKind,
    pub state: TaskState,
    /// Initialised lazily when the task becomes active.
    pub form:  Option<ResourceForm>,
}

impl WorkTask {
    pub fn new(kind: TaskKind) -> Self {
        Self { kind, state: TaskState::Pending, form: None }
    }

    /// Make this task active and initialise its form (idempotent).
    pub fn activate(&mut self, app_state: &AppState) {
        if self.form.is_none() {
            self.form = Some(self.kind.build_form(app_state));
        }
        self.state = TaskState::Active;
    }

    /// Human-readable label for the tab bar.
    pub fn label(&self, lang: Lang) -> &'static str {
        crate::i18n::t(lang, self.kind.label_key())
    }
}

// ── Task queue ────────────────────────────────────────────────────────────────

/// The wizard queue — always at least one task.
pub struct TaskQueue {
    pub tasks:  Vec<WorkTask>,
    /// Index of the task currently shown in the wizard.
    pub active: usize,
}

impl TaskQueue {
    /// Create a new queue with a single initial task (immediately activated).
    pub fn new(initial: TaskKind, state: &AppState) -> Self {
        let mut task = WorkTask::new(initial);
        task.activate(state);
        Self { tasks: vec![task], active: 0 }
    }

    pub fn active_task(&self) -> Option<&WorkTask> {
        self.tasks.get(self.active)
    }

    pub fn active_task_mut(&mut self) -> Option<&mut WorkTask> {
        self.tasks.get_mut(self.active)
    }

    /// Called after the active task's resource has been saved.
    ///
    /// Marks the task as Done, checks its required dependencies against the
    /// current app state, enqueues any unfulfilled ones, then advances the
    /// active pointer to the next pending/active task.
    ///
    /// Returns `true` if there are more tasks; `false` if the wizard is complete.
    pub fn on_task_saved(&mut self, app_state: &AppState) -> bool {
        // Collect new task kinds using an immutable borrow first
        let new_kinds: Vec<TaskKind> = {
            let task = match self.tasks.get(self.active) {
                Some(t) => t,
                None    => return false,
            };
            task.kind.required_deps()
                .iter()
                .copied()
                .filter(|&dep| !TaskKind::dep_fulfilled(dep, app_state))
                .filter_map(|dep| task.kind.dep_to_task(dep))
                .filter(|new_kind| {
                    !self.tasks.iter().any(|t| t.kind.same_variant(new_kind))
                })
                .collect()
        };

        // Now mutate: mark done, append new tasks
        if let Some(t) = self.tasks.get_mut(self.active) {
            t.state = TaskState::Done;
        }
        for kind in new_kinds {
            self.tasks.push(WorkTask::new(kind));
        }

        // Advance to the next non-done task
        if let Some(next) = self.tasks.iter().position(|t| t.state != TaskState::Done) {
            self.active = next;
            if let Some(t) = self.tasks.get_mut(next) {
                t.activate(app_state);
            }
            true
        } else {
            false
        }
    }
}
