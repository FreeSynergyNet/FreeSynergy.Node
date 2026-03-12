// Health validation — cross-resource consistency checks.
//
// Pattern: Validator (cross-resource) + Strategy (per-resource rule set).
//
// Each `check_*` function takes the primary resource config plus any
// related resources needed for cross-checks, and returns a `HealthStatus`.
//
// Health levels:
//   Ok      — all required conditions satisfied, deployment is possible.
//   Warning — optional components missing; deployment works but degraded.
//   Error   — required components missing; deployment will fail.
//
// Required vs. optional (per spec):
//   Project required:  host, proxy (via host), mail service, wiki service
//   Project optional:  monitoring service (→ Warning), git service (→ Warning)
//   Host required:     proxy configured, project assigned
//   Service required:  project assigned, host assigned

use crate::config::host::HostConfig;
use crate::config::project::{ProjectConfig, ServiceInstanceConfig};

// ── Health types ──────────────────────────────────────────────────────────────

/// Overall health level of a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum HealthLevel {
    #[default]
    Ok,
    /// Optional component missing — deployment works but is degraded.
    Warning,
    /// Required component missing — deployment will fail.
    Error,
}

impl HealthLevel {
    /// Single-character indicator for compact TUI display.
    pub fn indicator(self) -> &'static str {
        match self {
            HealthLevel::Ok      => "✓",
            HealthLevel::Warning => "⚠",
            HealthLevel::Error   => "✗",
        }
    }

    /// i18n key for the level label.
    pub fn i18n_key(self) -> &'static str {
        match self {
            HealthLevel::Ok      => "health.ok",
            HealthLevel::Warning => "health.warning",
            HealthLevel::Error   => "health.error",
        }
    }
}

/// A single health issue found during validation.
#[derive(Debug, Clone)]
pub struct HealthIssue {
    pub level:   HealthLevel,
    /// i18n key for the issue message (resolved by TUI via `i18n::t()`).
    pub msg_key: &'static str,
}

impl HealthIssue {
    fn error(msg_key: &'static str)   -> Self { Self { level: HealthLevel::Error,   msg_key } }
    fn warning(msg_key: &'static str) -> Self { Self { level: HealthLevel::Warning, msg_key } }
}

/// Aggregated health result for one resource.
#[derive(Debug, Clone, Default)]
pub struct HealthStatus {
    pub overall: HealthLevel,
    pub issues:  Vec<HealthIssue>,
}

impl HealthStatus {
    /// Create an Ok (no issues) status.
    pub fn ok() -> Self { Self::default() }

    fn push(&mut self, issue: HealthIssue) {
        if issue.level > self.overall {
            self.overall = issue.level;
        }
        self.issues.push(issue);
    }

    fn error(&mut self, key: &'static str)   { self.push(HealthIssue::error(key)); }
    fn warning(&mut self, key: &'static str) { self.push(HealthIssue::warning(key)); }

    /// `true` when there are no issues at all.
    pub fn is_ok(&self) -> bool { self.overall == HealthLevel::Ok }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract the broad type prefix from a service_class path.
/// E.g. `"mail/stalwart"` → `"mail"`, `"proxy/zentinel"` → `"proxy"`.
fn class_type(service_class: &str) -> &str {
    service_class.split('/').next().unwrap_or("")
}

/// Check whether a project's `load.services` contains at least one service
/// whose class path starts with the given type prefix.
fn project_has_type(project: &ProjectConfig, type_prefix: &str) -> bool {
    project.load.services.values()
        .any(|e| class_type(&e.service_class) == type_prefix)
}

// ── Project health ─────────────────────────────────────────────────────────────

/// Check the health of a project.
///
/// # Arguments
/// * `project`      — the project config to check.
/// * `host_projects` — list of project slugs referenced by known hosts
///                     (`host.host.project.as_deref()`). Used to verify the
///                     project has at least one host.
pub fn check_project(project: &ProjectConfig, host_projects: &[&str]) -> HealthStatus {
    let mut s = HealthStatus::ok();

    // ── Required: at least one host must reference this project ──────────────
    let has_host = host_projects.iter().any(|&p| p == project.project.meta.name.as_str());
    if !has_host {
        s.error("health.project.no_host");
    }

    // ── Required: mail service ────────────────────────────────────────────────
    if !project_has_type(project, "mail") {
        s.error("health.project.no_mail");
    }

    // ── Required: wiki service ────────────────────────────────────────────────
    if !project_has_type(project, "wiki") {
        s.error("health.project.no_wiki");
    }

    // ── Optional: monitoring ──────────────────────────────────────────────────
    if !project_has_type(project, "observability") && !project_has_type(project, "monitoring") {
        s.warning("health.project.no_monitoring");
    }

    // ── Optional: git hosting ─────────────────────────────────────────────────
    if !project_has_type(project, "git") {
        s.warning("health.project.no_git");
    }

    s
}

// ── Host health ────────────────────────────────────────────────────────────────

/// Check the health of a host.
pub fn check_host(host: &HostConfig) -> HealthStatus {
    let mut s = HealthStatus::ok();

    // ── Required: proxy must be configured ────────────────────────────────────
    if host.proxy.is_empty() {
        s.error("health.host.no_proxy");
    }

    // ── Required: must belong to a project ────────────────────────────────────
    let has_project = host.host.project.as_deref()
        .map(|p| !p.is_empty())
        .unwrap_or(false);
    if !has_project {
        s.error("health.host.no_project");
    }

    s
}

// ── Service health ─────────────────────────────────────────────────────────────

/// Check the health of a standalone service instance.
pub fn check_service(svc: &ServiceInstanceConfig) -> HealthStatus {
    let mut s = HealthStatus::ok();

    // ── Required: must belong to a project ────────────────────────────────────
    if svc.service.project.is_empty() {
        s.error("health.service.no_project");
    }

    // ── Required: must have a host assigned ───────────────────────────────────
    let has_host = svc.service.host.as_deref()
        .map(|h| !h.is_empty())
        .unwrap_or(false);
    if !has_host {
        s.error("health.service.no_host");
    }

    s
}
