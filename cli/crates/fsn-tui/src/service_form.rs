// Service-specific form — uses #[derive(Form)] for schema definition.
//
// Tabs:
//   Tab 0 (Service): name, class, version, tags
//   Tab 1 (Network): subdomain, alias, port
//   Tab 2 (Env):     env (EnvTableNode — key/value/comment rows)

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use fsn_form::Form;

use crate::app::{ResourceForm, ResourceKind, SERVICE_TABS};
use crate::schema_form;
use crate::ui::form_node::FormNode;

// ── Form data struct ──────────────────────────────────────────────────────────

/// Form schema for creating and editing a Service instance.
/// All fields live on a single tab (tab = 0).
#[derive(Form)]
pub struct ServiceFormData {
    #[form(label = "form.service.name", required, tab = 0, hint = "form.service.name.hint")]
    pub name: String,

    #[form(label = "form.service.class", widget = "select", required, tab = 0,
           options = "proxy/zentinel,iam/kanidm,mail/stalwart,git/forgejo,wiki/outline,chat/tuwunel,collab/cryptpad,tasks/vikunja,tickets/pretix,maps/umap,monitoring/openobserver,database/postgres,cache/dragonfly",
           default = "proxy/zentinel")]
    pub class: String,

    #[form(label = "form.options.version", tab = 0, default = "latest")]
    pub version: String,

    #[form(label = "form.service.tags", tab = 0, hint = "form.service.tags.hint")]
    pub tags: String,

    #[form(label = "form.service.subdomain", tab = 0, hint = "form.service.subdomain.hint")]
    pub subdomain: String,

    #[form(label = "form.service.alias", tab = 0, hint = "form.service.alias.hint")]
    pub alias: String,

    #[form(label = "form.service.port", tab = 0)]
    pub port: String,

    #[form(label = "form.service.env", widget = "env_table", tab = 0, rows = 6,
           hint = "form.service.env.hint")]
    pub env: String,
}

// ── Display helpers ───────────────────────────────────────────────────────────

pub fn service_class_display(code: &str) -> &'static str {
    match code {
        "proxy/zentinel"          => "Zentinel (Proxy)",
        "iam/kanidm"              => "Kanidm (IAM)",
        "mail/stalwart"           => "Stalwart (Mail)",
        "git/forgejo"             => "Forgejo (Git)",
        "wiki/outline"            => "Outline (Wiki)",
        "chat/tuwunel"            => "Tuwunel (Matrix/Chat)",
        "collab/cryptpad"         => "CryptPad (Collab)",
        "tasks/vikunja"           => "Vikunja (Tasks)",
        "tickets/pretix"          => "Pretix (Tickets)",
        "maps/umap"               => "uMap (Maps)",
        "monitoring/openobserver" => "OpenObserve (Monitoring)",
        "database/postgres"       => "PostgreSQL (Database)",
        "cache/dragonfly"         => "Dragonfly (Cache)",
        _                         => "—",
    }
}

const DISPLAY_FNS: &[(&str, fn(&str) -> &'static str)] = &[
    ("class", service_class_display),
];

// ── Smart-defaults hook ───────────────────────────────────────────────────────

fn service_on_change(nodes: &mut Vec<Box<dyn FormNode>>, key: &'static str) {
    if key == "name" {
        let name_val = nodes.iter().find(|n| n.key() == "name")
            .map(|n| n.value().to_string()).unwrap_or_default();
        let slug = crate::app::slugify(&name_val);

        let subdomain_dirty = nodes.iter().find(|n| n.key() == "subdomain")
            .map(|n| n.is_dirty()).unwrap_or(false);
        if !subdomain_dirty {
            if let Some(n) = nodes.iter_mut().find(|n| n.key() == "subdomain") {
                n.set_value(&slug);
            }
        }
    }

    // When the class changes, auto-populate env defaults from the plugin TOML
    // unless the user has already edited the env table manually.
    if key == "class" {
        let class = nodes.iter().find(|n| n.key() == "class")
            .map(|n| n.value().to_string()).unwrap_or_default();
        let env_dirty = nodes.iter().find(|n| n.key() == "env")
            .map(|n| n.is_dirty()).unwrap_or(false);
        if !env_dirty {
            if let Some(dir) = fsn_core::config::resolve_plugins_dir_no_fallback() {
                let defaults = load_class_env_defaults(&class, &dir);
                if !defaults.is_empty() {
                    if let Some(n) = nodes.iter_mut().find(|n| n.key() == "env") {
                        n.set_value(&defaults);
                    }
                }
            }
        }
    }
}

// ── Plugin defaults helper ────────────────────────────────────────────────────

/// Load the default environment variables from a container plugin class.
///
/// Returns "KEY=value\n..." ready for EnvTableNode::set_value(), or an empty
/// string when the plugin could not be loaded (no registry, unknown class).
///
/// The values are raw Jinja2 templates as written in the plugin TOML — the user
/// can review and adjust them before submitting the form.
pub fn load_class_env_defaults(class_key: &str, plugins_dir: &Path) -> String {
    fsn_core::config::ServiceRegistry::load(plugins_dir).ok()
        .and_then(|r| r.get(class_key).cloned())
        .map(|class| {
            class.environment.iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

// ── Form builders ─────────────────────────────────────────────────────────────

pub fn new_service_form() -> ResourceForm {
    let nodes = schema_form::build_nodes(
        ServiceFormData::schema(),
        &HashMap::new(),
        DISPLAY_FNS,
        &[],
        &[],
    );
    ResourceForm::new(ResourceKind::Service, SERVICE_TABS, nodes, None, service_on_change)
}

/// Build a service form with a pre-selected default class and optional env defaults.
///
/// `env_defaults` — raw "KEY=value\n..." string from `load_class_env_defaults()`.
/// Pass `None` (or an empty string) when no plugin defaults are available.
pub fn new_service_form_with_default_class(class: &str, env_defaults: Option<&str>) -> ResourceForm {
    let class_dyn = [("class", class.to_string())];
    let env_str   = env_defaults.unwrap_or("").to_string();
    let mut prefill = HashMap::new();
    if !env_str.is_empty() { prefill.insert("env", env_str.as_str()); }
    let nodes = schema_form::build_nodes(
        ServiceFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &class_dyn,
        &[],
    );
    ResourceForm::new(ResourceKind::Service, SERVICE_TABS, nodes, None, service_on_change)
}

/// Build a service form with a dynamic list of class options and optional env defaults.
///
/// `options` — class IDs (e.g. ["proxy/zentinel", "proxy/traefik"]).
/// `default` — pre-selected class (should be the first/local entry).
/// `env_defaults` — raw "KEY=value\n..." string from `load_class_env_defaults()`.
pub fn new_service_form_with_class_options(
    options:      Vec<String>,
    default:      &str,
    env_defaults: Option<&str>,
) -> ResourceForm {
    let class_dyn  = [("class", default.to_string())];
    let class_opts = [("class", options)];
    let env_str    = env_defaults.unwrap_or("").to_string();
    let mut prefill = HashMap::new();
    if !env_str.is_empty() { prefill.insert("env", env_str.as_str()); }
    let nodes = schema_form::build_nodes(
        ServiceFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &class_dyn,
        &class_opts,
    );
    ResourceForm::new(ResourceKind::Service, SERVICE_TABS, nodes, None, service_on_change)
}

// ── Submit ────────────────────────────────────────────────────────────────────

/// Write a standalone `.service.toml` file for the service instance.
///
/// `project_slug` is required: it populates the `project` field so the TOML
/// parses correctly as a `ServiceInstanceConfig`.
pub fn submit_service_form(form: &ResourceForm, services_dir: &Path, project_slug: &str) -> Result<()> {
    let name      = form.field_value("name");
    let class     = form.field_value("class");
    let version   = form.field_value("version");
    let tags_raw  = form.field_value("tags");
    let subdomain = form.field_value("subdomain");
    let alias     = form.field_value("alias");
    let port      = form.field_value("port");

    if name.is_empty()  { anyhow::bail!("Service name is required"); }
    if class.is_empty() { anyhow::bail!("Service class is required"); }

    let slug        = crate::app::slugify(&name);
    let version_val = if version.is_empty() { "latest".to_string() } else { version };
    let path        = services_dir.join(format!("{}.service.toml", slug));

    // Reject duplicate names — service names must be unique within a project.
    if path.exists() && form.edit_id.is_none() {
        anyhow::bail!("A service named '{}' already exists in this project", slug);
    }

    let mut content = format!(
        "[service]\nname          = \"{name}\"\nservice_class = \"{class}\"\nproject       = \"{project_slug}\"\nversion       = \"{version_val}\"\n"
    );

    if !subdomain.is_empty() {
        content.push_str(&format!("subdomain     = \"{subdomain}\"\n"));
    }
    if !alias.is_empty() {
        content.push_str(&format!("alias         = \"{alias}\"\n"));
    }
    if let Ok(p) = port.parse::<u16>() {
        content.push_str(&format!("port          = {p}\n"));
    }

    // Tags: CSV → TOML array
    let tags: Vec<String> = tags_raw.split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    if !tags.is_empty() {
        let tag_list = tags.iter().map(|t| format!("\"{t}\"")).collect::<Vec<_>>().join(", ");
        content.push_str(&format!("tags          = [{tag_list}]\n"));
    }

    // Env vars: "KEY=value\n..." → [environment] TOML table.
    // Comment lines (# ...) are UI-only metadata — not written to the file.
    let env_raw = form.field_value("env");
    let env_pairs: Vec<(String, String)> = env_raw.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { return None; }
            let (k, v) = line.split_once('=')?;
            let k = k.trim().to_string();
            if k.is_empty() { return None; }
            // Escape backslashes and double-quotes for TOML string literals.
            let v = v.trim().replace('\\', "\\\\").replace('"', "\\\"");
            Some((k, v))
        })
        .collect();
    if !env_pairs.is_empty() {
        content.push_str("\n[environment]\n");
        for (k, v) in &env_pairs {
            content.push_str(&format!("{k} = \"{v}\"\n"));
        }
    }

    std::fs::write(&path, content)?;
    Ok(())
}

/// Build a pre-filled service form for editing an existing service instance.
///
/// Reads from the in-memory `ServiceInstanceMeta` stored in the project config.
/// The service `name` is locked as the edit ID (slug-based).
pub fn edit_service_form(
    svc_name:  &str,
    svc_entry: &fsn_core::config::project::ServiceEntry,
    edit_slug: String,
) -> ResourceForm {
    let tags   = svc_entry.tags.join(", ");
    let port   = svc_entry.port.map(|p| p.to_string()).unwrap_or_default();
    let sub    = svc_entry.subdomain.as_deref().unwrap_or("");
    let alias  = svc_entry.alias.as_deref().unwrap_or("");
    let ver    = svc_entry.version.as_str();

    // Serialize env: IndexMap<String,String> → "KEY=value\n..." for EnvTableNode
    let env_str: String = svc_entry.env.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut prefill = HashMap::new();
    prefill.insert("name",      svc_name);
    prefill.insert("class",     svc_entry.service_class.as_str());
    prefill.insert("version",   ver);
    prefill.insert("tags",      tags.as_str());
    prefill.insert("subdomain", sub);
    prefill.insert("alias",     alias);
    prefill.insert("port",      port.as_str());
    prefill.insert("env",       env_str.as_str());

    let nodes = schema_form::build_nodes(
        ServiceFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &[],
        &[],
    );
    ResourceForm::new(ResourceKind::Service, SERVICE_TABS, nodes, Some(edit_slug), service_on_change)
}
