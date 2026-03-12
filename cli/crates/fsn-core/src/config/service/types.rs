// ServiceType enum — functional role classification for container plugins.
//
// Design Pattern: OOP — behavior belongs to the type itself.
//   ServiceType::exported_contract() → single source of truth for cross-service vars.
//   ServiceType::capabilities()      → what protocol/feature set a type guarantees.
//
// Separated from the class/meta structs so this enum (used everywhere for
// filtering and slot-matching) can be imported without pulling in the full
// container plugin definition (ContainerDef, HealthCheck, etc.).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── ServiceType ───────────────────────────────────────────────────────────────

/// The functional role of a service.
///
/// A service may declare **multiple types** — e.g. Zentinel is both `Proxy`
/// and `WebhosterSimple`; Keycloak is both `IamProvider` and `IamBroker`.
/// Types determine which project slots a service can fill and which
/// typed interfaces it exposes.
///
/// TOML accepts either a single string (legacy) or an array:
///   type   = "proxy"               # legacy / single
///   types  = ["proxy", "webhoster_simple"]   # multi-type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServiceType {
    // ── IAM ──────────────────────────────────────────────────────────────
    /// Identity provider: issues tokens, handles login (Kanidm, Keycloak, …)
    IamProvider,
    /// Identity broker: federates external identity providers (Keycloak, …)
    IamBroker,
    /// Legacy catch-all for any IAM service (mapped to IamProvider on read)
    Iam,

    // ── Proxy / Webhosting ────────────────────────────────────────────────
    /// Reverse proxy / ingress with TLS termination (Zentinel, Caddy, …)
    Proxy,
    /// Simple static or app webhosting (no PHP/FPM) (Zentinel, …)
    WebhosterSimple,

    // ── Communication ─────────────────────────────────────────────────────
    /// Mail server (Stalwart, …)
    Mail,
    /// Team chat / Matrix (Tuwunel, …)
    Chat,

    // ── Developer tools ───────────────────────────────────────────────────
    /// Git hosting (Forgejo, Gitea, …)
    Git,

    // ── Knowledge & collaboration ─────────────────────────────────────────
    /// Wiki / knowledge base (Outline, BookStack, …)
    Wiki,
    /// Collaborative editing (CryptPad, …)
    Collab,

    // ── Project management ────────────────────────────────────────────────
    /// Issue / task tracker (Vikunja, …)
    Tasks,
    /// Ticketing / event shop (Pretix, …)
    Tickets,

    // ── Geo & maps ────────────────────────────────────────────────────────
    /// Maps & geo (uMap, …)
    Maps,

    // ── Observability ─────────────────────────────────────────────────────
    /// Observability / metrics / logs (OpenObserver, …)
    Monitoring,

    // ── Infrastructure (internal) ─────────────────────────────────────────
    /// Relational database (Postgres) – internal, no proxy route
    Database,
    /// Key-value cache (Dragonfly/Redis) – internal, no proxy route
    Cache,

    // ── Bots / automation ─────────────────────────────────────────────────
    /// Bot / automation agent (Matrix bot, Telegram bot, …)
    Bot,

    /// User-defined / unknown type
    #[serde(other)]
    #[default]
    Custom,
}

impl std::fmt::Display for ServiceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ServiceType::IamProvider     => "iam_provider",
            ServiceType::IamBroker       => "iam_broker",
            ServiceType::Iam             => "iam",
            ServiceType::Proxy           => "proxy",
            ServiceType::WebhosterSimple => "webhoster_simple",
            ServiceType::Mail            => "mail",
            ServiceType::Chat            => "chat",
            ServiceType::Git             => "git",
            ServiceType::Wiki            => "wiki",
            ServiceType::Collab          => "collab",
            ServiceType::Tasks           => "tasks",
            ServiceType::Tickets         => "tickets",
            ServiceType::Maps            => "maps",
            ServiceType::Monitoring      => "monitoring",
            ServiceType::Database        => "database",
            ServiceType::Cache           => "cache",
            ServiceType::Bot             => "bot",
            ServiceType::Custom          => "custom",
        };
        write!(f, "{s}")
    }
}

impl ServiceType {
    /// Infer the primary `ServiceType` from a service class key prefix.
    ///
    /// Maps the first segment of a class key (e.g. "git" in "git/forgejo")
    /// to the corresponding ServiceType variant.  Used in pre-registry contexts
    /// (e.g. cross-service var collection) where the full class is not yet loaded.
    pub fn from_class_prefix(prefix: &str) -> Option<Self> {
        match prefix {
            "mail"       => Some(Self::Mail),
            "iam"        => Some(Self::IamProvider),
            "git"        => Some(Self::Git),
            "chat"       => Some(Self::Chat),
            "wiki"       => Some(Self::Wiki),
            "tasks"      => Some(Self::Tasks),
            "collab"     => Some(Self::Collab),
            "monitoring" => Some(Self::Monitoring),
            "tickets"    => Some(Self::Tickets),
            "maps"       => Some(Self::Maps),
            _            => None,
        }
    }

    /// Returns `true` for types that are internal infrastructure
    /// (no subdomain, no proxy route, no user-facing UI).
    pub fn is_internal(&self) -> bool {
        matches!(self, ServiceType::Database | ServiceType::Cache)
    }

    /// Returns `true` if this type can fill the IAM slot of a project.
    pub fn is_iam(&self) -> bool {
        matches!(self, ServiceType::IamProvider | ServiceType::IamBroker | ServiceType::Iam)
    }

    /// Returns `true` if this type can act as the project's reverse proxy.
    pub fn is_proxy(&self) -> bool {
        matches!(self, ServiceType::Proxy)
    }

    /// Logical category this type belongs to.
    ///
    /// Used for grouping in the service slot type-filter.
    /// Multiple types may share the same category (e.g. IamProvider + IamBroker → "iam").
    pub fn category(&self) -> &'static str {
        match self {
            ServiceType::IamProvider | ServiceType::IamBroker | ServiceType::Iam => "iam",
            ServiceType::Proxy | ServiceType::WebhosterSimple                    => "proxy",
            ServiceType::Mail  | ServiceType::Chat                               => "communication",
            ServiceType::Git                                                     => "developer",
            ServiceType::Wiki  | ServiceType::Collab                             => "knowledge",
            ServiceType::Tasks | ServiceType::Tickets                            => "project",
            ServiceType::Maps                                                    => "geo",
            ServiceType::Monitoring                                              => "monitoring",
            ServiceType::Database | ServiceType::Cache                          => "infrastructure",
            ServiceType::Bot                                                     => "automation",
            ServiceType::Custom                                                  => "custom",
        }
    }

    /// Human-readable label (English) for TUI display.
    pub fn label(&self) -> &'static str {
        match self {
            ServiceType::IamProvider     => "IAM Provider",
            ServiceType::IamBroker       => "IAM Broker",
            ServiceType::Iam             => "IAM",
            ServiceType::Proxy           => "Reverse Proxy",
            ServiceType::WebhosterSimple => "Webhoster (Simple)",
            ServiceType::Mail            => "Mail Server",
            ServiceType::Chat            => "Team Chat",
            ServiceType::Git             => "Git Hosting",
            ServiceType::Wiki            => "Wiki",
            ServiceType::Collab          => "Collaborative Editing",
            ServiceType::Tasks           => "Task Tracker",
            ServiceType::Tickets         => "Ticketing",
            ServiceType::Maps            => "Maps",
            ServiceType::Monitoring      => "Monitoring",
            ServiceType::Database        => "Database",
            ServiceType::Cache           => "Cache",
            ServiceType::Bot             => "Bot",
            ServiceType::Custom          => "Custom",
        }
    }

    /// One-sentence plain-language explanation of what this service type is.
    pub fn description(&self) -> &'static str {
        match self {
            ServiceType::IamProvider     => "Manages user identities, handles login and authentication (e.g. Kanidm, Keycloak).",
            ServiceType::IamBroker       => "Federates multiple identity providers — acts as a single login gateway.",
            ServiceType::Iam             => "Identity and access management service.",
            ServiceType::Proxy           => "Routes incoming HTTPS traffic to your services and handles TLS certificates automatically.",
            ServiceType::WebhosterSimple => "Hosts static websites or simple web apps directly without a separate proxy step.",
            ServiceType::Mail            => "Sends and receives emails for your domain (SMTP/IMAP).",
            ServiceType::Chat            => "Team messaging and real-time communication (Matrix protocol).",
            ServiceType::Git             => "Hosts Git repositories with web interface and CI/CD support.",
            ServiceType::Wiki            => "Collaborative knowledge base and documentation platform.",
            ServiceType::Collab          => "Real-time collaborative document and spreadsheet editing.",
            ServiceType::Tasks           => "Issue tracking and task management for your team.",
            ServiceType::Tickets         => "Event ticketing and shop system.",
            ServiceType::Maps            => "Self-hosted interactive maps and geo data.",
            ServiceType::Monitoring      => "Collects metrics, logs and traces — alerts when things go wrong.",
            ServiceType::Database        => "Relational database engine, internal use only (no public access).",
            ServiceType::Cache           => "In-memory key-value store for performance, internal use only.",
            ServiceType::Bot             => "Automation agent — connects services and handles scheduled tasks.",
            ServiceType::Custom          => "User-defined service type.",
        }
    }

    /// What contracts/interfaces this type provides to other services.
    ///
    /// Returns a list of human-readable strings explaining cross-service integration.
    pub fn what_it_provides(&self) -> &'static [&'static str] {
        match self {
            ServiceType::IamProvider => &[
                "Single Sign-On via OpenID Connect (OIDC)",
                "User directory for other services",
                "Auth token issuer",
            ],
            ServiceType::IamBroker => &[
                "Single Sign-On via OpenID Connect (OIDC)",
                "Federates external identity providers",
                "Auth token issuer",
            ],
            ServiceType::Iam => &[
                "Single Sign-On via OpenID Connect (OIDC)",
                "User directory for other services",
            ],
            ServiceType::Proxy => &[
                "HTTPS routing for all services",
                "Automatic TLS certificate management",
                "Health-based load balancing",
            ],
            ServiceType::WebhosterSimple => &[
                "Static file hosting without a separate proxy",
                "HTTPS routing for simple web apps",
            ],
            ServiceType::Mail => &[
                "MAIL_HOST, MAIL_DOMAIN, MAIL_URL, MAIL_PORT variables for other services",
                "SMTP relay for notifications and transactional email",
            ],
            ServiceType::Chat => &[
                "CHAT_HOST, CHAT_DOMAIN, CHAT_URL, CHAT_PORT variables",
                "Notification target for monitoring and automation",
            ],
            ServiceType::Git => &[
                "GIT_HOST, GIT_DOMAIN, GIT_URL, GIT_PORT variables",
                "OAuth2 provider for other services (if supported)",
            ],
            ServiceType::Wiki => &[
                "WIKI_HOST, WIKI_DOMAIN, WIKI_URL, WIKI_PORT variables",
            ],
            ServiceType::Collab => &[
                "COLLAB_HOST, COLLAB_DOMAIN, COLLAB_URL, COLLAB_PORT variables",
            ],
            ServiceType::Tasks => &[
                "TASKS_HOST, TASKS_DOMAIN, TASKS_URL, TASKS_PORT variables",
            ],
            ServiceType::Tickets => &[
                "TICKETS_HOST, TICKETS_DOMAIN, TICKETS_URL, TICKETS_PORT variables",
            ],
            ServiceType::Maps => &[
                "MAPS_HOST, MAPS_DOMAIN, MAPS_URL, MAPS_PORT variables",
            ],
            ServiceType::Monitoring => &[
                "MONITORING_HOST, MONITORING_DOMAIN, MONITORING_URL, MONITORING_PORT variables",
                "Centralized log and metric collection endpoint",
            ],
            ServiceType::Database => &[
                "Internal PostgreSQL connection (not exported to public)",
                "Used directly by services that need it",
            ],
            ServiceType::Cache => &[
                "Internal Redis-compatible connection (not exported)",
                "Used directly by services that need it",
            ],
            ServiceType::Bot => &[
                "Scheduled automation tasks",
                "Cross-service event handling",
            ],
            ServiceType::Custom => &[],
        }
    }

    /// Returns the cross-service variable contract for this type.
    ///
    /// `None` for internal/infrastructure types (Database, Cache, Proxy, Bot, Custom)
    /// that are not consumed directly by peer services via template variables.
    ///
    /// OOP principle: the type owns this knowledge, not the caller's match block.
    pub fn exported_contract(&self) -> Option<ExportedVarContract> {
        let prefix = match self {
            ServiceType::Mail                                                 => "MAIL",
            ServiceType::Iam | ServiceType::IamProvider | ServiceType::IamBroker => "IAM",
            ServiceType::Git                                                  => "GIT",
            ServiceType::Chat                                                 => "CHAT",
            ServiceType::Wiki                                                 => "WIKI",
            ServiceType::Tasks                                                => "TASKS",
            ServiceType::Collab                                               => "COLLAB",
            ServiceType::Monitoring                                           => "MONITORING",
            ServiceType::Tickets                                              => "TICKETS",
            ServiceType::Maps                                                 => "MAPS",
            // Internal / infrastructure: no cross-service export.
            ServiceType::Database | ServiceType::Cache
            | ServiceType::Proxy  | ServiceType::WebhosterSimple
            | ServiceType::Bot    | ServiceType::Custom                       => return None,
        };
        Some(ExportedVarContract { prefix })
    }

    /// Returns the base capability set guaranteed by every service of this type.
    ///
    /// Fine-grained capabilities (e.g. `IamScim`, `DatabaseMysql`) are declared
    /// at the plugin level in the container plugin TOML — these are the minimums
    /// that any implementation of this type must provide.
    pub fn capabilities(&self) -> Vec<Capability> {
        match self {
            ServiceType::Database | ServiceType::Cache         => vec![Capability::InternalOnly],
            ServiceType::Proxy | ServiceType::WebhosterSimple  => vec![Capability::InternalOnly, Capability::ProxyTls],
            ServiceType::IamProvider | ServiceType::Iam        => vec![Capability::IamOidc],
            ServiceType::IamBroker                             => vec![Capability::IamOidc, Capability::IamFederation],
            ServiceType::Mail                                   => vec![Capability::MailSmtp, Capability::MailImap],
            _                                                   => vec![],
        }
    }
}

// ── Capability ────────────────────────────────────────────────────────────────

/// Fine-grained protocol or feature capability.
///
/// ServiceType::capabilities() returns the guaranteed minimum for ALL plugins of
/// that type. Individual container plugins declare additional capabilities in their
/// TOML via `[module] capabilities = ["iam_scim", "database_postgres", …]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    // ── Infrastructure ─────────────────────────────────────────────────────
    /// Service has no proxy route and is not user-facing (Database, Cache, …)
    InternalOnly,

    // ── IAM ────────────────────────────────────────────────────────────────
    /// OpenID Connect / OAuth2 login
    IamOidc,
    /// LDAP directory access
    IamLdap,
    /// SAML 2.0 assertion
    IamSaml,
    /// SCIM 2.0 provisioning (Kanidm yes, Keycloak no)
    IamScim,
    /// Identity federation (brokering external IdPs)
    IamFederation,

    // ── Database ───────────────────────────────────────────────────────────
    /// PostgreSQL wire protocol
    DatabasePostgres,
    /// MySQL/MariaDB wire protocol
    DatabaseMysql,
    /// SQLite file-based storage
    DatabaseSqlite,

    // ── Cache ──────────────────────────────────────────────────────────────
    /// Redis RESP protocol (Dragonfly, Redis, KeyDB, …)
    CacheRedis,
    /// Memcached protocol
    CacheMemcached,

    // ── Mail ───────────────────────────────────────────────────────────────
    /// SMTP outbound + inbound
    MailSmtp,
    /// IMAP mailbox access
    MailImap,
    /// JMAP modern mail protocol
    MailJmap,

    // ── Proxy ──────────────────────────────────────────────────────────────
    /// Automatic TLS via ACME
    ProxyTls,
    /// DNS-01 ACME challenge support
    ProxyAcmeDns,

    /// Unknown / future capability — tolerated during TOML deserialization.
    #[serde(other)]
    Unknown,
}

// ── ExportedVarContract ───────────────────────────────────────────────────────

/// Defines what cross-service environment variables a service type exports.
///
/// All types that export vars use the same 4-variable pattern:
///   {PREFIX}_HOST    — container name (for internal DNS)
///   {PREFIX}_DOMAIN  — public subdomain (e.g. "mail.example.com")
///   {PREFIX}_URL     — full HTTPS URL (e.g. "https://mail.example.com")
///   {PREFIX}_PORT    — service port (from ServiceMeta::port)
///
/// This struct is the single source of truth — `desired.rs` calls
/// `contract.resolve(…)` instead of hard-coding prefix strings.
#[derive(Debug, Clone)]
pub struct ExportedVarContract {
    /// Variable prefix without trailing underscore (e.g. "MAIL", "IAM").
    pub prefix: &'static str,
}

impl ExportedVarContract {
    /// Resolve the contract into concrete key-value pairs.
    pub fn resolve(&self, name: &str, domain: &str, port: u16) -> HashMap<String, String> {
        let p = self.prefix;
        HashMap::from([
            (format!("{p}_HOST"),   name.to_string()),
            (format!("{p}_DOMAIN"), domain.to_string()),
            (format!("{p}_URL"),    format!("https://{domain}")),
            (format!("{p}_PORT"),   port.to_string()),
        ])
    }
}

// ── Multi-type deserializer ────────────────────────────────────────────────────

/// Deserialize `service_types` from either a single string or an array.
///
/// This enables backward-compatible reading of legacy TOML files that used
/// `type = "proxy"` alongside new files that use `types = ["proxy", "webhoster_simple"]`.
pub fn de_service_types<'de, D>(d: D) -> Result<Vec<ServiceType>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, SeqAccess, Visitor};
    use std::fmt;

    struct TypesVisitor;
    impl<'de> Visitor<'de> for TypesVisitor {
        type Value = Vec<ServiceType>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a service type string or array of service type strings")
        }
        // Single string: `type = "proxy"`
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            let t: ServiceType = serde::Deserialize::deserialize(
                serde::de::value::StrDeserializer::new(v)
            )?;
            Ok(vec![t])
        }
        // Array: `types = ["proxy", "webhoster_simple"]`
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut types = Vec::new();
            while let Some(t) = seq.next_element::<ServiceType>()? {
                types.push(t);
            }
            Ok(types)
        }
    }

    d.deserialize_any(TypesVisitor)
}
