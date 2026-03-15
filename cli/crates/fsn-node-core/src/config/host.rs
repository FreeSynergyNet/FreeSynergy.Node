use fsn_error::FsyError;
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

use crate::config::meta::ResourceMeta;

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
    /// Common fields: name, alias, description, version, tags.
    #[serde(flatten)]
    pub meta: ResourceMeta,

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

    /// Path to the SSH private key for deploy access (optional, falls back to ~/.ssh/id_ed25519).
    pub ssh_key_path: Option<String>,

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

    /// Convenience: returns name from embedded ResourceMeta.
    pub fn name(&self) -> &str { &self.meta.name }
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
    pub fn load(path: &Path) -> Result<Self, FsyError> {
        crate::config::load_toml_validated(path, crate::config::validate::TomlKind::Host)
    }
}

impl Resource for HostConfig {
    fn kind(&self) -> &'static str { "host" }
    fn id(&self) -> &str { self.host.addr() }
    fn display_name(&self) -> &str { self.host.meta.display_name() }
    fn tags(&self) -> &[String] { &self.host.meta.tags }

    fn validate(&self) -> Result<(), FsyError> {
        if self.host.meta.name.is_empty() { return Err(FsyError::Config("host.name is required".into())); }
        if self.host.addr().is_empty()    { return Err(FsyError::Config("host.address is required".into())); }
        Ok(())
    }
}

impl HostResource for HostConfig {
    fn addr(&self)        -> &str  { self.host.addr() }
    fn ssh_user(&self)    -> &str  { &self.host.ssh_user }
    fn ssh_port(&self)    -> u16   { self.host.ssh_port }
    fn is_external(&self) -> bool  { self.host.external }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::meta::ResourceMeta;

    #[test]
    fn host_config_parses_with_ssh_defaults() {
        let config: HostConfig = toml::from_str(r#"
[host]
name = "myhost"
address = "192.168.1.1"
        "#).unwrap();
        assert_eq!(config.host.meta.name, "myhost");
        assert_eq!(config.host.address, "192.168.1.1");
        assert_eq!(config.host.ssh_user, "root");
        assert_eq!(config.host.ssh_port, 22);
        assert!(config.proxy.is_empty());
    }

    #[test]
    fn host_meta_addr_prefers_address_over_ip() {
        let meta = HostMeta {
            meta: ResourceMeta { name: "test".to_string(), alias: None, description: None, version: "0.1.0".to_string(), tags: vec![] },
            address: "10.0.0.1".to_string(),
            project: None,
            install_dir: None,
            ssh_user: "root".to_string(),
            ssh_port: 22,
            ssh_key_path: None,
            ip: "192.168.1.1".to_string(),
            ipv6: String::new(),
            external: false,
        };
        assert_eq!(meta.addr(), "10.0.0.1");
    }

    #[test]
    fn host_meta_addr_falls_back_to_ip_when_address_empty() {
        let meta = HostMeta {
            meta: ResourceMeta { name: "test".to_string(), alias: None, description: None, version: "0.1.0".to_string(), tags: vec![] },
            address: String::new(),
            project: None,
            install_dir: None,
            ssh_user: "root".to_string(),
            ssh_port: 22,
            ssh_key_path: None,
            ip: "192.168.1.1".to_string(),
            ipv6: String::new(),
            external: false,
        };
        assert_eq!(meta.addr(), "192.168.1.1");
    }

    #[test]
    fn host_config_acme_provider_defaults_to_letsencrypt() {
        let config: HostConfig = toml::from_str(r#"
[host]
name = "myhost"
address = "192.168.1.1"

[acme]
email = "admin@example.com"
        "#).unwrap();
        assert_eq!(config.acme.as_ref().unwrap().provider, "letsencrypt");
    }

    #[test]
    fn host_config_parses_proxy_instance() {
        let config: HostConfig = toml::from_str(r#"
[host]
name = "myhost"
address = "192.168.1.1"

[proxy.zentinel]
service_class = "proxy/zentinel"
        "#).unwrap();
        assert!(config.proxy.contains_key("zentinel"));
        assert_eq!(config.proxy["zentinel"].service_class, "proxy/zentinel");
    }

    #[test]
    fn proxy_plugins_defaults_when_explicitly_empty_toml() {
        // serde default = fn only applies when the key is absent during deserialization,
        // not when Default::default() is invoked. Provide [load.plugins] to trigger it.
        let config: HostConfig = toml::from_str(r#"
[host]
name = "myhost"
address = "192.168.1.1"

[proxy.zentinel]
service_class = "proxy/zentinel"

[proxy.zentinel.load.plugins]
        "#).unwrap();
        let plugins = &config.proxy["zentinel"].load.plugins;
        assert_eq!(plugins.dns, "hetzner");
        assert_eq!(plugins.acme, "letsencrypt");
    }
}
