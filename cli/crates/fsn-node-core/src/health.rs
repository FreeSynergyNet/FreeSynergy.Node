// Health validation — OOP health checks via HealthCheck trait.
//
// Pattern: Strategy (HealthCheck trait) — each type knows its own invariants.
// Cross-resource checks (e.g. "is this project referenced by a host?") live
// in the free functions below since they require external context.
//
// Required vs. optional (per spec):
//   Project (self):    mail service, wiki service
//   Project (cross):   host reference, proxy (via check_project_with_hosts)
//   Project optional:  monitoring service (→ Warning), git service (→ Warning)
//   Host:              proxy configured, project assigned
//   Service:           project assigned, host assigned

use crate::config::host::HostConfig;
use crate::config::project::{ProjectConfig, ServiceInstanceConfig};

pub use fsn_health::{HealthCheck, HealthIssue, HealthLevel, HealthRules, HealthStatus};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn class_type(service_class: &str) -> &str {
    service_class.split('/').next().unwrap_or("")
}

fn project_has_type(project: &ProjectConfig, type_prefix: &str) -> bool {
    project.load.services.values()
        .any(|e| class_type(&e.service_class) == type_prefix)
}

// ── HealthCheck trait implementations ────────────────────────────────────────

impl HealthCheck for HostConfig {
    fn health(&self) -> HealthStatus {
        let has_project = self.host.project.as_deref()
            .map(|p| !p.is_empty())
            .unwrap_or(false);

        HealthRules::new()
            .require(!self.proxy.is_empty(), "health.host.no_proxy")
            .require(has_project,            "health.host.no_project")
            .build()
    }
}

impl HealthCheck for ServiceInstanceConfig {
    fn health(&self) -> HealthStatus {
        let has_host = self.service.host.as_deref()
            .map(|h| !h.is_empty())
            .unwrap_or(false);

        HealthRules::new()
            .require(!self.service.project.is_empty(), "health.service.no_project")
            .require(has_host,                         "health.service.no_host")
            .build()
    }
}

impl HealthCheck for ProjectConfig {
    /// Self-contained project health check (no cross-resource context).
    /// Use [`check_project_with_hosts`] for the full check including host reference.
    fn health(&self) -> HealthStatus {
        HealthRules::new()
            .require(project_has_type(self, "mail"), "health.project.no_mail")
            .require(project_has_type(self, "wiki"), "health.project.no_wiki")
            .warn(
                project_has_type(self, "observability") || project_has_type(self, "monitoring"),
                "health.project.no_monitoring",
            )
            .warn(project_has_type(self, "git"), "health.project.no_git")
            .build()
    }
}

// ── Cross-resource check ──────────────────────────────────────────────────────

/// Full project health check including cross-resource context.
///
/// Merges the self-contained checks from [`HealthCheck for ProjectConfig`]
/// with the host-reference check that requires external context.
///
/// # Arguments
/// * `project`       — the project config to check
/// * `host_projects` — project slugs referenced by all known hosts
pub fn check_project_with_hosts(project: &ProjectConfig, host_projects: &[&str]) -> HealthStatus {
    let has_host = host_projects.iter().any(|&p| p == project.project.meta.name.as_str());
    let self_status = project.health();

    let cross = HealthRules::new()
        .require(has_host, "health.project.no_host")
        .build();

    let mut combined = self_status;
    combined.merge(cross);
    combined
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::host::HostConfig;
    use crate::config::project::{ProjectConfig, ServiceInstanceConfig};

    fn host_with_proxy_and_project() -> HostConfig {
        toml::from_str(r#"
[host]
name = "myhost"
address = "192.168.1.1"
project = "myproject"

[proxy.zentinel]
service_class = "proxy/zentinel"
        "#).unwrap()
    }

    fn host_without_proxy() -> HostConfig {
        toml::from_str(r#"
[host]
name = "myhost"
address = "192.168.1.1"
        "#).unwrap()
    }

    fn host_without_project() -> HostConfig {
        toml::from_str(r#"
[host]
name = "myhost"
address = "192.168.1.1"

[proxy.zentinel]
service_class = "proxy/zentinel"
        "#).unwrap()
    }

    fn project_with_mail_and_wiki() -> ProjectConfig {
        toml::from_str(r#"
[project]
name = "myproject"
domain = "example.com"

[load.services.stalwart]
service_class = "mail/stalwart"

[load.services.outline]
service_class = "wiki/outline"
        "#).unwrap()
    }

    fn service_with_project_and_host() -> ServiceInstanceConfig {
        toml::from_str(r#"
[service]
name = "forgejo"
service_class = "git/forgejo"
project = "myproject"
host = "myhost"
        "#).unwrap()
    }

    #[test]
    fn host_health_ok_with_proxy_and_project() {
        let h = host_with_proxy_and_project();
        assert!(h.health().is_ok(), "host with proxy+project should be healthy");
    }

    #[test]
    fn host_health_error_without_proxy() {
        let status = host_without_proxy().health();
        assert_eq!(status.overall, HealthLevel::Error);
        assert!(status.issues.iter().any(|i| i.msg_key == "health.host.no_proxy"));
    }

    #[test]
    fn host_health_error_without_project() {
        let status = host_without_project().health();
        assert_eq!(status.overall, HealthLevel::Error);
        assert!(status.issues.iter().any(|i| i.msg_key == "health.host.no_project"));
    }

    #[test]
    fn service_health_ok_with_project_and_host() {
        assert!(service_with_project_and_host().health().is_ok());
    }

    #[test]
    fn service_health_error_without_host() {
        let s: ServiceInstanceConfig = toml::from_str(r#"
[service]
name = "forgejo"
service_class = "git/forgejo"
project = "myproject"
        "#).unwrap();
        let status = s.health();
        assert_eq!(status.overall, HealthLevel::Error);
        assert!(status.issues.iter().any(|i| i.msg_key == "health.service.no_host"));
    }

    #[test]
    fn service_health_error_without_project() {
        let s: ServiceInstanceConfig = toml::from_str(r#"
[service]
name = "forgejo"
service_class = "git/forgejo"
project = ""
host = "myhost"
        "#).unwrap();
        let status = s.health();
        assert_eq!(status.overall, HealthLevel::Error);
        assert!(status.issues.iter().any(|i| i.msg_key == "health.service.no_project"));
    }

    #[test]
    fn project_health_warns_without_monitoring_and_git() {
        let status = project_with_mail_and_wiki().health();
        // Has mail + wiki → no errors; missing monitoring + git → warnings
        assert_eq!(status.overall, HealthLevel::Warning);
        assert!(!status.issues.iter().any(|i| i.level == HealthLevel::Error));
        assert!(status.issues.iter().any(|i| i.msg_key == "health.project.no_monitoring"));
        assert!(status.issues.iter().any(|i| i.msg_key == "health.project.no_git"));
    }

    #[test]
    fn project_health_error_without_mail() {
        let p: ProjectConfig = toml::from_str(r#"
[project]
name = "myproject"
domain = "example.com"

[load.services.outline]
service_class = "wiki/outline"
        "#).unwrap();
        let status = p.health();
        assert!(status.issues.iter().any(|i| i.msg_key == "health.project.no_mail"));
    }

    #[test]
    fn project_health_error_without_wiki() {
        let p: ProjectConfig = toml::from_str(r#"
[project]
name = "myproject"
domain = "example.com"

[load.services.stalwart]
service_class = "mail/stalwart"
        "#).unwrap();
        let status = p.health();
        assert!(status.issues.iter().any(|i| i.msg_key == "health.project.no_wiki"));
    }

    #[test]
    fn check_project_with_hosts_ok_when_referenced() {
        let p = project_with_mail_and_wiki();
        let status = check_project_with_hosts(&p, &["myproject"]);
        assert!(!status.issues.iter().any(|i| i.msg_key == "health.project.no_host"));
    }

    #[test]
    fn check_project_with_hosts_error_when_no_host_references_it() {
        let p = project_with_mail_and_wiki();
        let status = check_project_with_hosts(&p, &[]);
        assert!(status.issues.iter().any(|i| i.msg_key == "health.project.no_host"));
    }
}
