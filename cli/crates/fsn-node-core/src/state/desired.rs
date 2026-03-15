// Desired state – what SHOULD be running according to project + host config.

use std::collections::HashMap;

use crate::config::service::{Capability, ServiceClass, ServiceType};
use crate::resource::VarProvider;

/// The fully resolved desired state for a project on a host.
#[derive(Debug, Clone)]
pub struct DesiredState {
    pub project_name: String,
    pub domain: String,
    /// Top-level service instances (sub-services nested inside).
    pub services: Vec<ServiceInstance>,
}

/// A resolved service instance – the class with all Jinja2 vars expanded.
#[derive(Debug, Clone)]
pub struct ServiceInstance {
    /// Instance name (e.g. "forgejo") – unique per project.
    pub name: String,

    /// Service class key (e.g. "git/forgejo").
    pub class_key: String,

    /// The class template this instance was resolved from.
    pub class: ServiceClass,

    /// Functional types (convenience copy from class.meta.service_types).
    pub service_types: Vec<ServiceType>,

    /// Jinja2-expanded environment variables (ready for Quadlet .env file).
    pub resolved_env: HashMap<String, String>,

    /// The full subdomain this service listens on (e.g. "forgejo.example.com").
    pub service_domain: String,

    /// Alias subdomains (CNAME targets).
    pub alias_domains: Vec<String>,

    /// Sub-services owned by this instance (e.g. postgres, dragonfly).
    pub sub_services: Vec<ServiceInstance>,

    /// Version from the class definition (used to detect updates).
    pub version: String,

    /// Jinja2-expanded volume mount strings (ready for Quadlet Volume= lines).
    /// Empty when resolved without a data_root (non-deploy contexts).
    pub resolved_volumes: Vec<String>,

    /// Merged capability set: type defaults + plugin-declared extras.
    /// Empty for sub-services and types without a known capability set.
    pub capabilities: Vec<Capability>,
}

// ── VarProvider impl ──────────────────────────────────────────────────────────

impl VarProvider for ServiceInstance {
    /// Exports cross-service variables based on service types.
    ///
    /// Delegates to `ServiceType::exported_contract()` — the type itself is
    /// the single source of truth for which variables it exports and with
    /// which prefix. This eliminates the match block that previously lived here.
    ///
    /// Internal services (Database, Cache, Proxy, Bot) have no contract and
    /// export nothing — they are not consumed via template variables by peers.
    fn exported_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        for t in &self.service_types {
            if let Some(contract) = t.exported_contract() {
                vars.extend(contract.resolve(
                    &self.name,
                    &self.service_domain,
                    self.class.meta.port,
                ));
            }
        }
        vars
    }
}
