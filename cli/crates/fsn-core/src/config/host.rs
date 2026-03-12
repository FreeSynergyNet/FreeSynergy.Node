// Host config – maps to projects/{project}/{hostname}.host.toml
//
// Rules (per RULES.md):
//   - One file per physical/virtual host
//   - Proxy is ALWAYS defined here, never in project.toml
//   - Every host MUST have a proxy service
//   - DNS/ACME at host level = default for all services on that host
//   - Proxy-level DNS/ACME overrides the host default

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::FsnError;
use crate::resource::{HostResource, Resource};

/// Root structure of a host config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostConfig {
    pub host:  HostMeta,

    /// Proxy service declaration — required on every host.
    #[serde(default)]
    pub proxy: IndexMap<String, ProxyInstance>,

    /// Host-level DNS default (used by all services unless overridden).
    pub dns:  Option<HostDns>,

    /// Host-level ACME/TLS default.
    pub acme: Option<HostAcme>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostMeta {
    pub name: String,

    /// Display alias, e.g. "main", "backup".
    pub alias: Option<String>,

    /// Primary IPv4 address or FQDN.
    pub address: String,

    /// Which project this host belongs to (project slug).
    pub project: Option<String>,

    /// Base install directory on this host (overrides project default).
    pub install_dir: Option<String>,

    /// SSH username for Ansible / deploy access.
    #[serde(default = "default_ssh_user")]
    pub ssh_user: String,

    /// SSH port.
    #[serde(default = "default_ssh_port")]
    pub ssh_port: u16,

    /// Free-form tags for grouping / filtering.
    #[serde(default)]
    pub tags: Vec<String>,

    // ── Legacy fields (kept for backward compat) ──────────────────────────────

    /// Legacy: IPv4 — prefer `address`.
    #[serde(default)]
    pub ip: String,

    /// IPv6 address (optional).
    #[serde(default)]
    pub ipv6: String,

    /// true = no SSH, externally managed host.
    #[serde(default)]
    pub external: bool,
}

fn default_ssh_user() -> String { "root".into() }
fn default_ssh_port() -> u16   { 22 }

impl HostMeta {
    /// Returns the canonical address: `address` if set, falls back to legacy `ip`.
    pub fn addr(&self) -> &str {
        if !self.address.is_empty() { &self.address } else { &self.ip }
    }
}

/// Host-level DNS provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostDns {
    /// Provider name: "cloudflare" | "hetzner" | "manual".
    pub provider: String,

    /// Reference to the API token in vault (vault_* key).
    pub token_ref: Option<String>,

    /// DNS zones managed by this token.
    #[serde(default)]
    pub zones: Vec<String>,
}

/// Host-level ACME/TLS configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostAcme {
    /// Contact email for Let's Encrypt / ACME.
    pub email: String,

    /// ACME provider: "letsencrypt" | "zerossl" | "buypass" | "none".
    #[serde(default = "default_acme_provider")]
    pub provider: String,
}

fn default_acme_provider() -> String { "letsencrypt".into() }

/// A proxy instance declaration (typically "zentinel").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyInstance {
    pub service_class: String,

    #[serde(default)]
    pub load: ProxyLoad,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProxyLoad {
    #[serde(default)]
    pub plugins: ProxyPlugins,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProxyPlugins {
    /// DNS provider: "hetzner" | "cloudflare" | "none"
    #[serde(default = "default_dns")]
    pub dns: String,

    /// ACME provider: "letsencrypt" | "smallstep-ca" | "none"
    #[serde(default = "default_acme")]
    pub acme: String,

    /// ACME contact email (overrides host-level acme.email)
    pub acme_email: Option<String>,
}

fn default_dns()  -> String { "hetzner".into() }
fn default_acme() -> String { "letsencrypt".into() }

impl HostConfig {
    /// Load a host config from a TOML file.
    pub fn load(path: &Path) -> Result<Self, FsnError> {
        crate::config::load_toml_validated(path, crate::config::validate::TomlKind::Host)
    }
}

impl Resource for HostConfig {
    fn kind(&self) -> &'static str { "host" }
    fn id(&self) -> &str { self.host.addr() }
    fn display_name(&self) -> &str {
        self.host.alias.as_deref().unwrap_or(&self.host.name)
    }
    fn tags(&self) -> &[String] { &self.host.tags }

    fn validate(&self) -> Result<(), FsnError> {
        if self.host.name.is_empty()   { return Err(FsnError::ConstraintViolation { message: "host.name is required".into() }); }
        if self.host.addr().is_empty() { return Err(FsnError::ConstraintViolation { message: "host.address is required".into() }); }
        Ok(())
    }
}

impl HostResource for HostConfig {
    fn addr(&self)        -> &str  { self.host.addr() }
    fn ssh_user(&self)    -> &str  { &self.host.ssh_user }
    fn ssh_port(&self)    -> u16   { self.host.ssh_port }
    fn is_external(&self) -> bool  { self.host.external }
}
