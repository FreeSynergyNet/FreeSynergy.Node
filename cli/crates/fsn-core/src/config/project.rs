// Project config – maps to projects/{name}/{name}.project.toml
//
// Naming convention (per RULES.md):
//   {name}.project.toml     → local deployment (this machine)
//   {name}.{host}.toml      → remote host deployment
//   {name}.federation.toml  → federation config

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use toml::Value;

use crate::error::FsnError;
use crate::resource::{ProjectResource, Resource, ServiceResource};

/// Root structure of a project config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project: ProjectMeta,

    /// Typed service slots – which instance fills each role.
    #[serde(default)]
    pub services: ServiceSlots,

    #[serde(default)]
    pub load: ProjectLoad,
}

// ── Service Slots ─────────────────────────────────────────────────────────────

/// Typed service slots at the project level.
/// Other services and bots use these to find the right instance.
///
/// In project.toml:
/// [services]
/// iam  = "kanidm"
/// mail = "stalwart"
/// wiki = "outline"
/// git  = "forgejo"
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServiceSlots {
    pub iam:        Option<String>,
    pub mail:       Option<String>,
    pub wiki:       Option<String>,
    pub git:        Option<String>,
    pub chat:       Option<String>,
    pub collab:     Option<String>,
    pub tasks:      Option<String>,
    pub monitoring: Option<String>,
    #[serde(default, flatten)]
    pub extra: IndexMap<String, String>,
}

// ── Project Metadata ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub domain: String,
    pub description: Option<String>,

    /// Project version – increment to trigger config re-generation.
    #[serde(default = "default_version")]
    pub version: String,

    /// Primary language (IETF tag, e.g. "en", "de").
    #[serde(default = "default_lang")]
    pub language: String,

    /// Additional supported languages (ordered by preference).
    #[serde(default)]
    pub languages: Vec<String>,

    /// Base installation directory on the host (e.g. "/opt/fsn" or "~/fsn").
    /// Overrides the host-level default when set.
    #[serde(default)]
    pub install_dir: Option<String>,

    /// Free-form tags (e.g. for filtering or categorisation).
    #[serde(default)]
    pub tags: Vec<String>,

    pub contact: Option<ContactInfo>,
    pub branding: Option<BrandingConfig>,
    pub sites: Option<IndexMap<String, SiteConfig>>,
}

fn default_version() -> String { "0.1.0".into() }
fn default_lang()    -> String { "en".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInfo {
    pub email: Option<String>,
    pub acme_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingConfig {
    pub path: String,
    pub logo: Option<String>,
    pub logo_dark: Option<String>,
    pub favicon: Option<String>,
    pub theme_css: Option<String>,
    pub bg_pattern: Option<String>,
    pub meta: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub path: String,
    pub domain: Option<String>,
}

// ── Load (instance declarations) ──────────────────────────────────────────────

/// The [load] table – which service instances to deploy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectLoad {
    /// key = instance name (e.g. "forgejo"), value = service entry.
    /// Alias "modules" accepted for backward compatibility with existing project files.
    #[serde(default, alias = "modules")]
    pub services: IndexMap<String, ServiceEntry>,
}

/// A service instance declaration inside a project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    /// Service class path, e.g. "git/forgejo".
    /// Alias "module_class" accepted for backward compatibility.
    #[serde(alias = "module_class")]
    pub service_class: String,

    /// Display alias, also used as subdomain override.
    pub alias: Option<String>,

    /// Which host slug this service runs on.
    pub host: Option<String>,

    /// Subdomain prefix → {subdomain}.{project.domain}. Defaults to instance name.
    pub subdomain: Option<String>,

    /// Port override (uses service-class default when absent).
    pub port: Option<u16>,

    /// Image version / tag.
    #[serde(default = "default_service_version")]
    pub version: String,

    /// Free-form tags.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Instance-level environment variable overrides.
    /// Merged on top of the service class's [environment] block during resolution.
    #[serde(default)]
    pub env: IndexMap<String, String>,

    #[serde(default)]
    pub vars: IndexMap<String, Value>,
}

fn default_service_version() -> String { "latest".into() }

/// Backwards-compat alias.
pub type ModuleRef = ServiceEntry;

// ── Standalone service instance file ──────────────────────────────────────────

/// Full service instance config stored in its own file.
/// Maps to: projects/{project}/services/{name}.service.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInstanceConfig {
    pub service: ServiceInstanceMeta,

    #[serde(default)]
    pub vars: IndexMap<String, Value>,
}

/// Metadata block inside a standalone service instance file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInstanceMeta {
    /// Instance name (unique within the project).
    pub name: String,

    /// Service class path, e.g. "git/forgejo".
    pub service_class: String,

    /// Which project this service belongs to (project slug).
    pub project: String,

    /// Display alias; also used as subdomain when set.
    pub alias: Option<String>,

    /// Which host slug this service runs on.
    pub host: Option<String>,

    /// Subdomain prefix → {subdomain}.{project.domain}.
    pub subdomain: Option<String>,

    /// Port override (uses service-class default when absent).
    pub port: Option<u16>,

    #[serde(default = "default_version")]
    pub version: String,

    #[serde(default)]
    pub tags: Vec<String>,

    /// Git repository of the deployed code (optional metadata).
    pub git_repo: Option<String>,

    /// Public website URL (optional metadata).
    pub website: Option<String>,

    /// Bot names attached to this service.
    #[serde(default)]
    pub bots: Vec<String>,
}

impl ServiceInstanceConfig {
    pub fn load(path: &std::path::Path) -> Result<Self, crate::error::FsnError> {
        crate::config::load_toml_validated(path, crate::config::validate::TomlKind::Service)
    }
}

impl Resource for ServiceInstanceConfig {
    fn kind(&self) -> &'static str { "service" }
    fn id(&self) -> &str { &self.service.name }
    fn display_name(&self) -> &str {
        self.service.alias.as_deref().unwrap_or(&self.service.name)
    }
    fn tags(&self) -> &[String] { &self.service.tags }

    fn validate(&self) -> Result<(), FsnError> {
        if self.service.name.is_empty() {
            return Err(FsnError::ConstraintViolation { message: "service.name is required".into() });
        }
        if self.service.service_class.is_empty() {
            return Err(FsnError::ConstraintViolation { message: "service.service_class is required".into() });
        }
        if self.service.project.is_empty() {
            return Err(FsnError::ConstraintViolation { message: "service.project is required".into() });
        }
        Ok(())
    }
}

impl ServiceResource for ServiceInstanceConfig {
    fn service_class(&self) -> &str { &self.service.service_class }
    fn host(&self)          -> Option<&str> { self.service.host.as_deref() }
    fn subdomain(&self)     -> Option<&str> { self.service.subdomain.as_deref() }
    fn port(&self)          -> Option<u16>  { self.service.port }
    fn project(&self)       -> &str { &self.service.project }
}

impl ProjectConfig {
    pub fn load(path: &Path) -> Result<Self, FsnError> {
        crate::config::load_toml_validated(path, crate::config::validate::TomlKind::Project)
    }
}

impl Resource for ProjectConfig {
    fn kind(&self) -> &'static str { "project" }
    fn id(&self) -> &str { &self.project.name }
    fn display_name(&self) -> &str { &self.project.name }
    fn description(&self) -> Option<&str> { self.project.description.as_deref() }

    fn validate(&self) -> Result<(), FsnError> {
        if self.project.name.is_empty() {
            return Err(FsnError::ConstraintViolation { message: "project.name is required".into() });
        }
        if self.project.domain.is_empty() {
            return Err(FsnError::ConstraintViolation { message: "project.domain is required".into() });
        }
        Ok(())
    }
}

impl ProjectResource for ProjectConfig {
    fn domain(&self) -> &str { &self.project.domain }
    fn contact_email(&self) -> Option<&str> {
        self.project.contact.as_ref()
            .and_then(|c| c.email.as_deref().or(c.acme_email.as_deref()))
    }
    fn languages(&self) -> &[String] { &self.project.languages }
    fn install_dir(&self) -> Option<&str> { self.project.install_dir.as_deref() }
}
