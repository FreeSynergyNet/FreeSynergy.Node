use fsn_error::FsyError;
// Service class definition – maps to modules/{type}/{name}/{name}.toml
//
// Design Pattern: Module split:
//   types.rs     — ServiceType enum + de_service_types (role classification)
//   mod.rs       — ServiceMeta, ServiceClass, ContainerDef, ServiceContract,
//                  ServiceLifecycle, setup types, …
//
// Field order (MANDATORY per RULES.md):
//   module → vars → load → container → environment → setup → lifecycle
//
// The TOML key `[module]` is kept for file-level compatibility;
// internally we use `ServiceMeta` / `ServiceClass`.

pub mod types;

pub use types::{Capability, ExportedVarContract, ServiceType, de_service_types};

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use toml::Value;

// ── Schema helpers ─────────────────────────────────────────────────────────────

/// JSON-Schema helper for `IndexMap<String, toml::Value>` fields.
/// `toml::Value` has no JsonSchema impl — we accept any JSON object here.
fn schema_any_object(_: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
    schemars::schema::Schema::Bool(true)
}

use crate::config::manifest::ModuleManifest;

use crate::resource::Resource;

// ── Service Class ─────────────────────────────────────────────────────────────

/// A service class definition (the template/blueprint for a service).
/// Loaded from modules/{type}/{name}/{name}.toml.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServiceClass {
    /// Metadata block – TOML key is `[module]` for file compatibility.
    #[serde(rename = "module")]
    pub meta: ServiceMeta,

    /// Jinja2 template variables declared by the module.
    /// Any TOML value is accepted; validated at render time.
    #[serde(default)]
    #[schemars(schema_with = "schema_any_object")]
    pub vars: IndexMap<String, Value>,

    #[serde(default)]
    pub load: ServiceLoad,

    pub container: ContainerDef,

    #[serde(default)]
    pub environment: IndexMap<String, String>,

    /// Setup wizard configuration – what this service needs before it can run.
    #[serde(default)]
    pub setup: ServiceSetup,

    /// Routing contract – what the service exposes to the proxy.
    /// Proxy modules iterate over all contracts to generate routing config.
    #[serde(default)]
    pub contract: ServiceContract,

    /// Plugin manifest – commands, inputs and outputs for the process plugin protocol.
    /// Absent for modules that have not yet been migrated to the plugin system.
    #[serde(default, rename = "plugin")]
    #[schemars(schema_with = "schema_any_object")]
    pub manifest: Option<ModuleManifest>,

    /// Lifecycle hooks — what to do on install, update, swap, decommission.
    #[serde(default)]
    pub lifecycle: ServiceLifecycle,
}

// ── Service Contract ──────────────────────────────────────────────────────────

/// Routing and capability contract declared by a service module.
///
/// The proxy driver reads `ServiceContract` to generate per-service routing
/// config — analogous to a Kubernetes `Ingress` spec.  The service declares
/// what it needs; the proxy decides how to implement it.
///
/// Empty `routes` = no proxy routing generated (internal services).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ServiceContract {
    /// HTTP routes this service exposes. Empty = proxy skips this service.
    #[serde(default)]
    pub routes: Vec<RouteSpec>,

    /// Extra HTTP headers the proxy injects when forwarding to this service.
    #[serde(default)]
    pub headers: Vec<HeaderSpec>,

    /// Whether the container speaks TLS internally.
    /// `true` → proxy uses HTTPS to reach the container (e.g. Kanidm).
    /// `false` (default) → proxy speaks plain HTTP to the container.
    #[serde(default)]
    pub upstream_tls: bool,

    /// Override the proxy health-check path for this service.
    /// Falls back to `module.health_path` when absent.
    pub health_path: Option<String>,
}

/// A URL route this service exposes through the proxy.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RouteSpec {
    /// Unique identifier within this module (e.g. "main", "admin", "api").
    pub id: String,

    /// URL path prefix to match (e.g. "/" or "/auth").
    pub path: String,

    /// Strip the matched prefix before forwarding to the upstream.
    #[serde(default)]
    pub strip: bool,

    /// Human-readable description (shown in TUI and generated docs).
    pub description: Option<String>,
}

/// An HTTP header the proxy injects when forwarding requests.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HeaderSpec {
    /// Header name (e.g. "X-Forwarded-Proto").
    pub name: String,
    /// Header value — Jinja2 templates allowed (e.g. "{{ service_domain }}").
    pub value: String,
}

// ── Setup wizard types ────────────────────────────────────────────────────────

/// All configuration fields this service requires during `fsn init`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ServiceSetup {
    #[serde(default)]
    pub fields: Vec<SetupField>,
}

/// A single field the wizard will prompt for.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SetupField {
    /// Key to set: "vault_*" → stored in vault, anything else → env reminder.
    pub key: String,

    /// English label shown in prompt AND used as .po lookup key.
    pub label: String,

    /// Optional longer explanation shown below the prompt.
    pub description: Option<String>,

    #[serde(default)]
    pub field_type: FieldType,

    /// Auto-generate a random value; user can press Enter to accept or type override.
    #[serde(default)]
    pub auto_generate: bool,

    /// Pre-filled default value shown in the prompt.
    pub default: Option<String>,

    /// For FieldType::Select – the available choices.
    #[serde(default)]
    pub options: Vec<String>,

    /// Skip this field if the key already exists in vault (idempotent).
    #[serde(default = "default_true")]
    pub skip_if_set: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    #[default]
    String,
    Secret,  // masked input, stored in vault
    Email,
    Ip,
    Select,  // requires `options`
    Bool,
}

// ── Service Metadata ──────────────────────────────────────────────────────────

/// Core metadata declared under the `[module]` TOML key.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServiceMeta {
    pub name: String,

    #[serde(default)]
    pub alias: Vec<String>,

    /// Functional types – determines typed interfaces and project slots.
    ///
    /// Accepts either `type = "proxy"` (legacy single string) or
    /// `types = ["proxy", "webhoster_simple"]` (multi-type array).
    /// Both keys are accepted; `types` takes precedence if both are present.
    #[serde(
        rename = "types",
        alias = "type",
        default,
        deserialize_with = "de_service_types"
    )]
    #[schemars(with = "Vec<ServiceType>", rename = "types")]
    pub service_types: Vec<ServiceType>,

    pub author: Option<String>,
    pub version: String,

    #[serde(default)]
    pub tags: Vec<String>,

    pub description: Option<String>,
    pub website: Option<String>,
    pub repository: Option<String>,

    /// Primary internal port the service listens on.
    pub port: u16,

    #[serde(default)]
    pub constraints: Constraints,

    pub federation: Option<FederationMeta>,

    /// Path used by Zentinel upstream health checks.
    pub health_path: Option<String>,
    pub health_port: Option<u16>,
    pub health_scheme: Option<String>,

    /// Fine-grained capabilities this plugin provides beyond the type defaults.
    /// Example: `capabilities = ["iam_scim", "iam_ldap"]` in the plugin TOML.
    #[serde(default)]
    pub capabilities: Vec<Capability>,

    /// Service role declarations — which roles this module provides / requires.
    #[serde(default)]
    pub roles: ModuleRoles,

    /// UI integration hints — how the Desktop should open this service.
    #[serde(default)]
    pub ui: ModuleUi,
}

// ── ModuleRoles ───────────────────────────────────────────────────────────────

/// Service role declarations embedded in `[module.roles]`.
///
/// Roles are MIME-like identifiers for system functions (e.g. "proxy", "iam").
/// `provides` lists what this module can fulfil.
/// `requires` lists what must be assigned before this module will work.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ModuleRoles {
    /// Role IDs this module can fulfil (e.g. `["proxy", "webhoster"]`).
    #[serde(default)]
    pub provides: Vec<String>,

    /// Role IDs this module depends on being fulfilled by another service.
    #[serde(default)]
    pub requires: Vec<String>,
}

// ── ModuleUi ──────────────────────────────────────────────────────────────────

/// Desktop UI hints embedded in `[module.ui]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ModuleUi {
    /// Whether this service has a web UI that can be opened in the Desktop browser.
    #[serde(default)]
    pub supports_web: bool,

    /// How the Desktop opens this service: `"tab"` (default), `"window"`, `"embed"`.
    pub open_mode: Option<String>,

    /// Jinja2 template for the service URL (e.g. `"https://{{ service_domain }}"`).
    pub web_url_template: Option<String>,
}

impl ServiceMeta {
    /// Returns `true` if this service is purely internal infrastructure
    /// (no subdomain, no proxy route, no user-facing UI).
    /// Requires ALL declared types to be internal.
    pub fn is_internal_only(&self) -> bool {
        !self.service_types.is_empty()
            && self.service_types.iter().all(|t| t.is_internal())
    }

    /// Returns `true` if any of the declared types matches `t`.
    pub fn has_type(&self, t: &ServiceType) -> bool {
        self.service_types.contains(t)
    }

    /// The primary type (first in the list), or `Custom` if the list is empty.
    pub fn primary_type(&self) -> &ServiceType {
        self.service_types.first().unwrap_or(&ServiceType::Custom)
    }

    /// Comma-separated label list for TUI display (e.g. "Reverse Proxy, Webhoster (Simple)").
    pub fn types_label(&self) -> String {
        if self.service_types.is_empty() {
            return ServiceType::Custom.label().to_string();
        }
        self.service_types.iter().map(|t| t.label()).collect::<Vec<_>>().join(", ")
    }
}

/// Deployment constraints declared per service class.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Constraints {
    /// Maximum number of instances of this service class per host (null = unlimited).
    pub per_host: Option<u32>,

    /// Maximum number of instances of this service class per IP (null = unlimited).
    pub per_ip: Option<u32>,

    /// Locality constraint – if Some(SameHost), must run on same host as consumer.
    pub locality: Option<Locality>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Locality {
    SameHost,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FederationMeta {
    pub enabled: bool,
    pub min_trust: u8,
}

// ── Load / Dependencies ───────────────────────────────────────────────────────

/// Sub-service and service references declared under `[load]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ServiceLoad {
    /// Sub-services this service owns and creates (e.g. postgres, dragonfly).
    /// TOML key: `modules` kept for file compatibility.
    #[serde(default, alias = "modules")]
    pub sub_services: IndexMap<String, SubServiceRef>,

    /// Other services whose config this service reads (no ownership).
    #[serde(default)]
    pub services: IndexMap<String, ServiceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubServiceRef {
    /// Class key, e.g. "database/postgres".
    /// TOML: `module_class` or `service_class` (both accepted).
    #[serde(alias = "module_class")]
    pub service_class: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ServiceRef {}

// ── Container Definition ──────────────────────────────────────────────────────

/// Container definition – maps to the `[container]` TOML block.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ContainerDef {
    pub name: String,
    pub image: String,
    pub image_tag: String,

    /// Auto-generated by engine – NEVER set manually in service TOML.
    #[serde(default)]
    pub networks: Vec<String>,

    #[serde(default)]
    pub volumes: Vec<String>,

    /// Forbidden on all services except proxy/zentinel.
    #[serde(default)]
    pub published_ports: Vec<String>,

    pub healthcheck: Option<HealthCheck>,

    /// Run as a specific UID[:GID] (e.g. "1000" or "15371:15371").
    pub user: Option<String>,

    #[serde(default)]
    pub read_only: bool,

    #[serde(default)]
    pub tmpfs: Vec<String>,

    #[serde(default)]
    pub security_opt: Vec<String>,

    /// Resource limits (ulimit key → value). Any TOML value accepted.
    #[serde(default)]
    #[schemars(schema_with = "schema_any_object")]
    pub ulimits: IndexMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HealthCheck {
    pub cmd: String,
    pub interval: String,
    pub timeout: String,
    pub retries: u32,
    pub start_period: String,
}

// ── Service Lifecycle ─────────────────────────────────────────────────────────

/// Lifecycle hooks declared under `[lifecycle]` in a module TOML.
///
/// Phases run in order:  init → install → configure → start → running
///                       running → update → backup → migrate → swap → decommission
///
/// Each hook is idempotent by design — the engine may re-run it on reconcile.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ServiceLifecycle {
    /// Hooks that fire after successful installation.
    #[serde(default)]
    pub on_install: Vec<LifecycleHook>,

    /// Hooks that fire when another service is installed alongside this one.
    /// Each entry declares which peer type triggers it (`trigger = "wiki.*"`).
    #[serde(default)]
    pub on_peer_install: Vec<PeerHook>,

    /// Hooks that fire before and after an update (new image pull).
    #[serde(default)]
    pub on_update: Vec<LifecycleHook>,

    /// Hooks that fire during a swap (this service is being replaced).
    #[serde(default)]
    pub on_swap: Vec<LifecycleHook>,

    /// Hooks that fire during decommission (graceful shutdown + data archival).
    #[serde(default)]
    pub on_decommission: Vec<LifecycleHook>,
}

/// A single lifecycle hook action.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LifecycleHook {
    /// What to do.
    pub action: LifecycleAction,

    /// Shell command (for `run` action). Tera-template variables are supported.
    pub command: Option<String>,

    /// Bus event name (for `bus_emit` action).
    pub event: Option<String>,

    /// Payload to emit with the bus event (arbitrary TOML table).
    #[serde(default)]
    #[schemars(schema_with = "schema_any_object")]
    pub data: IndexMap<String, Value>,

    /// Target path or label (for `backup` / `export` actions).
    pub target: Option<String>,

    /// Export format: `"json"` | `"toml"` | `"tar"` (for `export` action).
    pub format: Option<String>,
}

/// A lifecycle hook that fires when a specific peer service type is installed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PeerHook {
    /// Glob-style trigger pattern matching the peer's primary ServiceType label.
    /// Examples: `"wiki.*"`, `"git/forgejo"`, `"iam.*"`.
    pub trigger: String,

    /// Action to take when the trigger matches.
    pub action: LifecycleAction,

    /// Shell command (for `run` action).
    pub command: Option<String>,

    /// Script arguments (positional).
    #[serde(default)]
    pub args: Vec<String>,
}

/// Discriminant for lifecycle hook actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleAction {
    /// Run a shell command inside the container (via `podman exec`).
    Run,
    /// Emit a named event onto the FSN service bus.
    BusEmit,
    /// Create a data backup before proceeding.
    Backup,
    /// Export service data to a portable format for consumption by another service.
    Export,
}

impl ServiceLifecycle {
    /// Returns `true` if no hooks are defined (all fields empty).
    pub fn is_empty(&self) -> bool {
        self.on_install.is_empty()
            && self.on_peer_install.is_empty()
            && self.on_update.is_empty()
            && self.on_swap.is_empty()
            && self.on_decommission.is_empty()
    }

    /// Returns the peer hooks whose trigger matches the given service type label.
    /// `peer_type_label` is the primary type label of the newly installed peer.
    pub fn matching_peer_hooks(&self, peer_type_label: &str) -> Vec<&PeerHook> {
        self.on_peer_install
            .iter()
            .filter(|h| glob_matches(&h.trigger, peer_type_label))
            .collect()
    }
}

/// Minimal glob matcher: `*` matches anything within a single path segment,
/// `.*` at the end matches any sub-type.
fn glob_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" || pattern == value {
        return true;
    }
    // Pattern like "wiki.*" → prefix match on "wiki/"
    if let Some(prefix) = pattern.strip_suffix(".*") {
        return value.starts_with(&format!("{prefix}/")) || value == prefix;
    }
    false
}

// ── Resource impl for ServiceClass ────────────────────────────────────────────

impl Resource for ServiceClass {
    fn kind(&self) -> &'static str { "service_class" }
    fn id(&self) -> &str { &self.meta.name }
    fn display_name(&self) -> &str { &self.meta.name }
    fn description(&self) -> Option<&str> { self.meta.description.as_deref() }
    fn tags(&self) -> &[String] { &self.meta.tags }

    fn validate(&self) -> Result<(), FsyError> {
        if self.meta.name.is_empty() {
            return Err(FsyError::Config("module.name is required".into()));
        }
        if self.meta.version.is_empty() {
            return Err(FsyError::Config("module.version is required".into()));
        }
        if self.container.image.is_empty() {
            return Err(FsyError::Config("container.image is required".into()));
        }
        Ok(())
    }
}
