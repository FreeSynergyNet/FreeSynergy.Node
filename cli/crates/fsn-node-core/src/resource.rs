// Resource hierarchy — the OOP foundation of FreeSynergy.Node.
//
// Analogous to the HTML DOM element hierarchy:
//   Element → HTMLElement → HTMLInputElement
//   Resource → ProjectResource / HostResource / ServiceResource / BotResource
//
// Generic tooling (dashboard, validator, deploy pipeline) uses `dyn Resource`.
// Type-specific tooling uses the extended traits: `dyn ProjectResource`, etc.
//
// Every top-level managed object implements at least `Resource`:
//   Project, Host, Service (instance), ServiceClass, Bot

use std::collections::HashMap;
use std::fmt;
use fsn_error::FsyError;


// ── Lifecycle phase ───────────────────────────────────────────────────────────

/// Lifecycle phase of a managed resource.
/// Mirrors Kubernetes condition types for familiarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResourcePhase {
    /// Phase unknown — not yet queried from the runtime (Podman / Ansible).
    #[default]
    Unknown,
    /// Config present but not yet deployed.
    Pending,
    /// All conditions satisfied; resource is fully operational.
    Ready,
    /// Running but one or more conditions are degraded.
    Degraded,
    /// Deployment failed or resource is in an error state.
    Failed,
}

impl fmt::Display for ResourcePhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourcePhase::Unknown  => write!(f, "unknown"),
            ResourcePhase::Pending  => write!(f, "pending"),
            ResourcePhase::Ready    => write!(f, "ready"),
            ResourcePhase::Degraded => write!(f, "degraded"),
            ResourcePhase::Failed   => write!(f, "failed"),
        }
    }
}

// ── Base Resource trait ───────────────────────────────────────────────────────

/// Base interface for all FSN-managed resources.
///
/// Analogous to `Element` in the HTML DOM: every managed type (Project, Host,
/// Service, Bot) implements this trait. Generic tooling uses `dyn Resource` for
/// type-agnostic operations: listing, filtering by tag, or batch validation.
///
/// For type-specific operations, cast to the appropriate sub-trait:
/// - [`ProjectResource`] — domain, languages, contact
/// - [`HostResource`]    — address, SSH config, external flag
/// - [`ServiceResource`] — service class, host, subdomain, port
/// - [`BotResource`]     — project, integration type
pub trait Resource: fmt::Debug {
    /// Machine-readable type tag.
    /// One of: `"project"` | `"host"` | `"service"` | `"service_class"` | `"bot"`
    fn kind(&self) -> &'static str;

    /// Unique identifier within the resource's namespace (slug, name, address, …).
    fn id(&self) -> &str;

    /// Human-readable display name (may contain spaces and mixed case).
    /// Defaults to [`id()`](Resource::id) when not overridden.
    fn display_name(&self) -> &str { self.id() }

    /// Optional one-line description of what this resource does.
    fn description(&self) -> Option<&str> { None }

    /// Free-form tags for filtering and grouping (e.g. `["production", "eu-west"]`).
    fn tags(&self) -> &[String] { &[] }

    /// Validate all structural invariants.
    ///
    /// Returns `Ok(())` when the resource is self-consistent.
    /// Returns `Err(FsyError::ConstraintViolation)` on the first violation found.
    fn validate(&self) -> Result<(), FsyError>;

    /// Current lifecycle phase.
    ///
    /// Requires a live runtime query (Podman / Ansible). Returns
    /// [`ResourcePhase::Unknown`] by default until an external reconciler observes
    /// the actual state.
    fn phase(&self) -> ResourcePhase { ResourcePhase::Unknown }
}

// ── Type-specific sub-traits ──────────────────────────────────────────────────

/// Project-specific resource interface.
///
/// Extends [`Resource`] with project domain knowledge.
/// Use `&dyn ProjectResource` when project-level properties are needed
/// but the concrete type is unknown at compile time.
pub trait ProjectResource: Resource {
    /// Primary domain name, e.g. `"example.com"`.
    fn domain(&self) -> &str;

    /// Contact / ACME email address, if configured.
    fn contact_email(&self) -> Option<&str>;

    /// Supported languages in order of preference (IETF tags, e.g. `["en", "de"]`).
    fn languages(&self) -> &[String];

    /// Base installation directory on the host, e.g. `"/opt/fsn"`.
    fn install_dir(&self) -> Option<&str>;
}

/// Host-specific resource interface.
///
/// Extends [`Resource`] with connectivity and infrastructure properties.
pub trait HostResource: Resource {
    /// Canonical network address: IPv4, IPv6, or FQDN.
    fn addr(&self) -> &str;

    /// SSH username used for Ansible / deploy access.
    fn ssh_user(&self) -> &str;

    /// SSH port (typically 22).
    fn ssh_port(&self) -> u16;

    /// `true` = externally managed; FSN performs no SSH on this host.
    fn is_external(&self) -> bool;
}

/// Service instance interface.
///
/// Extends [`Resource`] with deployment properties for a running service instance.
pub trait ServiceResource: Resource {
    /// Service class path, e.g. `"git/forgejo"`.
    fn service_class(&self) -> &str;

    /// Host slug this service is deployed on, if specified.
    fn host(&self) -> Option<&str>;

    /// Subdomain prefix: `{subdomain}.{project.domain}`.
    fn subdomain(&self) -> Option<&str>;

    /// Port override (uses the service class default when `None`).
    fn port(&self) -> Option<u16>;

    /// Project slug this service belongs to.
    fn project(&self) -> &str;
}

// ── VarProvider ───────────────────────────────────────────────────────────────

/// Provides deploy-time variables to the shared template context.
///
/// Each resource knows which variables it exports to other services in the same
/// deploy context. The engine collects all providers and merges their maps before
/// Jinja2 rendering, enabling automatic cross-service injection without hardcoded lookups.
///
/// # Naming convention (SCREAMING_SNAKE_CASE with type prefix)
/// - Project:    `PROJECT_NAME`, `PROJECT_DOMAIN`, `PROJECT_EMAIL`
/// - Host:       `HOST_ADDR`, `HOST_INSTALL_DIR`
/// - Mail:       `MAIL_HOST`, `MAIL_DOMAIN`, `MAIL_URL`, `MAIL_PORT`
/// - IAM:        `IAM_HOST`, `IAM_DOMAIN`, `IAM_URL`
/// - Git:        `GIT_HOST`, `GIT_DOMAIN`, `GIT_URL`
/// - (pattern continues for Chat, Wiki, Tasks, Collab, Monitoring, …)
pub trait VarProvider {
    /// Variables this resource exports to the deploy template context.
    fn exported_vars(&self) -> HashMap<String, String>;
}

/// Bot / automation agent interface.
///
/// Extends [`Resource`] for lightweight automation agents attached to a project.
/// Examples: Matrix bot (Hookshot), Telegram bot, generic webhook receiver.
pub trait BotResource: Resource {
    /// Project slug this bot belongs to.
    fn project(&self) -> &str;

    /// Service class that runs this bot, e.g. `"bot/matrix-hookshot"`.
    fn service_class(&self) -> &str;

    /// Machine-readable integration type: `"matrix"` | `"telegram"` | `"webhook"` | `"custom"`.
    fn bot_type_str(&self) -> &str;
}
