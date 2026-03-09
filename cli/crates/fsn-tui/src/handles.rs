// Runtime wrappers for loaded config files.
//
// Each handle combines the on-disk config with filesystem metadata (slug, path)
// and implements the Resource trait hierarchy from fsn-core.

use std::path::PathBuf;

use anyhow::Result;

use fsn_core::config::host::HostConfig;
use fsn_core::config::project::{ProjectConfig, ServiceInstanceConfig};
use fsn_core::error::FsnError;
use fsn_core::resource::{HostResource, ProjectResource, Resource, ServiceResource};
pub use fsn_core::state::actual::RunState;

// ── Project handle ────────────────────────────────────────────────────────────

/// Runtime wrapper for a loaded project config.
///
/// Combines the on-disk `ProjectConfig` with the filesystem slug (derived from
/// the filename stem) and the absolute path to the TOML file.
#[derive(Debug, Clone)]
pub struct ProjectHandle {
    /// Filesystem slug — derived from `{name}.project.toml` filename stem.
    pub slug:      String,
    /// Absolute path to the `.project.toml` file.
    pub toml_path: PathBuf,
    /// Parsed project configuration.
    pub config:    ProjectConfig,
}

impl ProjectHandle {
    pub fn name(&self)        -> &str { &self.config.project.name }
    pub fn domain(&self)      -> &str { &self.config.project.domain }
    pub fn install_dir(&self) -> &str {
        self.config.project.install_dir.as_deref().unwrap_or("")
    }
    pub fn email(&self) -> &str {
        self.config.project.contact.as_ref()
            .and_then(|c| c.email.as_deref().or(c.acme_email.as_deref()))
            .unwrap_or("")
    }
}

impl Resource for ProjectHandle {
    fn kind(&self) -> &'static str { "project" }
    fn id(&self)   -> &str         { &self.slug }
    fn display_name(&self) -> &str { &self.config.project.name }
    fn description(&self)  -> Option<&str> { self.config.project.description.as_deref() }
    fn validate(&self) -> Result<(), FsnError> { self.config.validate() }
}

impl ProjectResource for ProjectHandle {
    fn domain(&self)        -> &str           { &self.config.project.domain }
    fn contact_email(&self) -> Option<&str>   { self.config.contact_email() }
    fn languages(&self)     -> &[String]      { &self.config.project.languages }
    fn install_dir(&self)   -> Option<&str>   { self.config.project.install_dir.as_deref() }
}

// ── Host handle ───────────────────────────────────────────────────────────────

/// Runtime wrapper for a loaded host config.
///
/// Combines the on-disk `HostConfig` with the filesystem slug and absolute path.
#[derive(Debug, Clone)]
pub struct HostHandle {
    /// Filesystem slug — derived from `{name}.host.toml` filename stem.
    pub slug:      String,
    /// Absolute path to the `.host.toml` file.
    pub toml_path: PathBuf,
    /// Parsed host configuration.
    pub config:    HostConfig,
}

impl HostHandle {
    pub fn name(&self) -> &str { &self.config.host.name }
    pub fn addr(&self) -> &str { self.config.host.addr() }
}

impl Resource for HostHandle {
    fn kind(&self) -> &'static str { "host" }
    fn id(&self)   -> &str         { &self.slug }
    fn display_name(&self) -> &str {
        self.config.host.alias.as_deref().unwrap_or(&self.config.host.name)
    }
    fn tags(&self)  -> &[String]  { &self.config.host.tags }
    fn validate(&self) -> Result<(), FsnError> { self.config.validate() }
}

impl HostResource for HostHandle {
    fn addr(&self)        -> &str  { self.config.host.addr() }
    fn ssh_user(&self)    -> &str  { &self.config.host.ssh_user }
    fn ssh_port(&self)    -> u16   { self.config.host.ssh_port }
    fn is_external(&self) -> bool  { self.config.host.external }
}

// ── Service instance handle ────────────────────────────────────────────────────

/// Runtime wrapper for a loaded service instance config.
///
/// Combines the on-disk `ServiceInstanceConfig` with the filesystem slug and path.
#[derive(Debug, Clone)]
pub struct ServiceHandle {
    /// Instance name — derived from `{name}.service.toml` filename stem.
    pub name:      String,
    /// Absolute path to the `.service.toml` file.
    pub toml_path: PathBuf,
    /// Parsed service instance configuration.
    pub config:    ServiceInstanceConfig,
}

impl Resource for ServiceHandle {
    fn kind(&self) -> &'static str { "service" }
    fn id(&self)   -> &str         { &self.name }
    fn display_name(&self) -> &str {
        self.config.service.alias.as_deref().unwrap_or(&self.name)
    }
    fn tags(&self)  -> &[String]  { &self.config.service.tags }
    fn validate(&self) -> Result<(), FsnError> { self.config.validate() }
}

impl ServiceResource for ServiceHandle {
    fn service_class(&self) -> &str         { &self.config.service.service_class }
    fn host(&self)          -> Option<&str> { self.config.service.host.as_deref() }
    fn subdomain(&self)     -> Option<&str> { self.config.service.subdomain.as_deref() }
    fn port(&self)          -> Option<u16>  { self.config.service.port }
    fn project(&self)       -> &str         { &self.config.service.project }
}

// ── Service table row ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ServiceRow {
    pub name:         String,
    pub service_type: String,
    pub domain:       String,
    pub status:       RunState,
}

/// i18n key for a run state — delegates to `RunState::i18n_key()`.
#[inline]
pub fn run_state_i18n(state: RunState) -> &'static str {
    state.i18n_key()
}
