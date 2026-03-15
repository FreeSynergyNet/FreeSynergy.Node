pub mod bot;
pub mod discovery;
pub mod host;
pub mod manifest;
pub mod meta;
pub mod plugin;
pub mod project;
pub mod registry;
pub mod service;
pub mod settings;
pub mod validate;
pub mod vault;

pub use bot::{BotConfig, BotMeta, BotType};
pub use host::{HostConfig, HostDns, HostAcme, HostMeta};
pub use meta::ResourceMeta;
pub use manifest::{
    ModuleManifest, ManifestInputs, ManifestOutputFile,
    PluginContext, InstanceInfo, PeerService, PeerRoute,
    PluginResponse, OutputFile, ShellCommand, LogLine, LogLevel,
};
pub use service::{
    Capability, ExportedVarContract,
    Constraints, ContainerDef, Locality,
    ServiceClass, ServiceMeta, ServiceType,
    ServiceLoad, ServiceSetup, SetupField, FieldType,
    SubServiceRef, ServiceRef,
    ServiceContract, RouteSpec, HeaderSpec,
    ModuleRoles, ModuleUi,
};
pub use project::{
    ModuleRef,       // backwards-compat alias
    ProjectConfig, ProjectLoad, ProjectMeta,
    ServiceEntry, ServiceSlots,
    ServiceInstanceConfig, ServiceInstanceMeta,
};
pub use plugin::{PluginConfig, PluginMeta};
pub use registry::ServiceRegistry;
pub use discovery::{find_project, find_host, find_host_by_name};
pub use settings::{AppSettings, StoreConfig, ServiceRoleRegistry, resolve_plugins_dir, resolve_plugins_dir_no_fallback};
pub use vault::VaultConfig;

// ── Shared TOML loader ────────────────────────────────────────────────────────

/// Load and deserialize any TOML config file into `T`.
///
/// Single source of truth for the read-and-parse pattern used by all config
/// types (`ProjectConfig`, `HostConfig`, `ServiceInstanceConfig`, …).
/// Returns typed `FsyError` variants so callers do not need to map manually.
pub fn load_toml<T>(path: &std::path::Path) -> Result<T, fsn_error::FsyError>
where
    T: serde::de::DeserializeOwned,
{
    load_toml_validated(path, validate::TomlKind::Generic)
}

/// Load and deserialize a TOML config file with schema + safety validation.
///
/// Chain of Responsibility:
///   1. Read file
///   2. validate::validate_toml_content (size → syntax → safety → schema)
///   3. Deserialize into `T`
pub fn load_toml_validated<T>(
    path: &std::path::Path,
    kind: validate::TomlKind,
) -> Result<T, fsn_error::FsyError>
where
    T: serde::de::DeserializeOwned,
{
    let path_str = path.display().to_string();
    let content = std::fs::read_to_string(path)
        .map_err(|_| fsn_error::FsyError::NotFound(path_str.clone()))?;
    validate::validate_toml_content(&content, kind, &path_str)?;
    toml::from_str(&content)
        .map_err(|e| fsn_error::FsyError::Parse(format!("{path_str}: {e}")))
}
