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
    // ── Section: Basis ────────────────────────────────────────────────────
    #[form(widget = "section", label = "form.section.basis", tab = 0)]
    pub _section_basis: String,

    #[form(label = "form.service.name", required, tab = 0, col = 7, min_w = 28,
           hint = "form.service.name.hint")]
    pub name: String,

    #[form(label = "form.service.class", widget = "select", required, tab = 0, col = 5, min_w = 22,
           options = "proxy/zentinel,iam/kanidm,mail/stalwart,git/forgejo,wiki/outline,chat/tuwunel,collab/cryptpad,tasks/vikunja,tickets/pretix,maps/umap,monitoring/openobserver,database/postgres,cache/dragonfly",
           default = "proxy/zentinel")]
    pub class: String,

    /// Project this service belongs to (required).
    #[form(label = "form.service.project", widget = "select", required, tab = 0, col = 6, min_w = 22)]
    pub project: String,

    /// Host this service runs on (required).
    #[form(label = "form.service.host", widget = "select", required, tab = 0, col = 6, min_w = 22)]
    pub host: String,

    // ── Section: Details ────────────────────────────────────────────────
    #[form(widget = "section", label = "form.section.details", tab = 0)]
    pub _section_details: String,

    #[form(label = "form.service.alias", tab = 0, col = 6, min_w = 22,
           hint = "form.service.alias.hint")]
    pub alias: String,

    #[form(label = "form.service.subdomain", tab = 0, col = 6, min_w = 22,
           hint = "form.service.subdomain.hint")]
    pub subdomain: String,

    #[form(label = "form.options.version", tab = 0, col = 4, min_w = 14, default = "latest")]
    pub version: String,

    #[form(label = "form.service.port", tab = 0, col = 4, min_w = 14)]
    pub port: String,

    #[form(label = "form.service.external", widget = "select", tab = 0, col = 4, min_w = 14,
           options = "false,true", default = "false")]
    pub external: String,

    #[form(label = "form.service.description", widget = "textarea", rows = 3, tab = 0,
           hint = "form.service.description.hint")]
    pub description: String,

    #[form(label = "form.service.tags", tab = 0, col = 6, min_w = 18,
           hint = "form.service.tags.hint")]
    pub tags: String,

    #[form(label = "form.service.git_repo", tab = 0, col = 6, min_w = 22,
           hint = "form.service.git_repo.hint")]
    pub git_repo: String,

    #[form(label = "form.service.website", tab = 0, col = 6, min_w = 22)]
    pub website: String,

    // ── Section: Environment ────────────────────────────────────────────
    #[form(widget = "section", label = "form.section.environment", tab = 0)]
    pub _section_env: String,

    #[form(label = "form.service.env", widget = "env_table", tab = 0, rows = 6,
           hint = "form.service.env.hint")]
    pub env: String,
}

// ── Display helpers ───────────────────────────────────────────────────────────

pub fn service_class_display(code: &str) -> String {
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
        _                         => return code.to_string(),
    }.to_string()
}

const DISPLAY_FNS: &[(&str, fn(&str) -> String)] = &[
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

pub fn new_service_form(
    project_slugs: Vec<String>,
    host_slugs:    Vec<String>,
    current_project: &str,
    current_host:    &str,
) -> ResourceForm {
    let mut prefill = HashMap::new();
    if !current_project.is_empty() { prefill.insert("project", current_project); }
    if !current_host.is_empty()    { prefill.insert("host", current_host); }
    let dyn_opts = vec![
        ("project", project_slugs),
        ("host",    host_slugs),
    ];
    let mut nodes = schema_form::build_nodes(
        ServiceFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &[],
        &dyn_opts,
    );
    // Seed env defaults for the initial class (proxy/zentinel) immediately —
    // service_on_change only fires on user interactions, so we trigger it once here.
    service_on_change(&mut nodes, "class");
    ResourceForm::new(ResourceKind::Service, SERVICE_TABS, nodes, None, service_on_change)
}

/// Build a service form with class options loaded from the store.
///
/// Uses all available store entries as dropdown options.
/// Falls back to the static schema list when the store is empty (offline / not yet synced).
pub fn new_service_form_from_store(
    store_entries:   &[fsn_core::store::StoreEntry],
    project_slugs:   Vec<String>,
    host_slugs:      Vec<String>,
    current_project: &str,
    current_host:    &str,
) -> ResourceForm {
    if store_entries.is_empty() {
        return new_service_form(project_slugs, host_slugs, current_project, current_host);
    }
    let options: Vec<String> = store_entries.iter().map(|e| e.id.clone()).collect();
    let default = options[0].clone();
    let env_defaults = fsn_core::config::resolve_plugins_dir_no_fallback()
        .map(|dir| load_class_env_defaults(&default, &dir))
        .filter(|s| !s.is_empty());
    new_service_form_with_class_options(options, &default, env_defaults.as_deref(), project_slugs, host_slugs, current_project, current_host)
}

/// Build a service form with a pre-selected default class and optional env defaults.
///
/// `env_defaults` — raw "KEY=value\n..." string from `load_class_env_defaults()`.
/// Pass `None` (or an empty string) when no plugin defaults are available.
pub fn new_service_form_with_default_class(
    class:           &str,
    env_defaults:    Option<&str>,
    project_slugs:   Vec<String>,
    host_slugs:      Vec<String>,
    current_project: &str,
    current_host:    &str,
) -> ResourceForm {
    let class_dyn = [("class", class.to_string())];
    let env_str   = env_defaults.unwrap_or("").to_string();
    let mut prefill = HashMap::new();
    if !env_str.is_empty()         { prefill.insert("env", env_str.as_str()); }
    if !current_project.is_empty() { prefill.insert("project", current_project); }
    if !current_host.is_empty()    { prefill.insert("host", current_host); }
    let dyn_opts = vec![
        ("project", project_slugs),
        ("host",    host_slugs),
    ];
    let nodes = schema_form::build_nodes(
        ServiceFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &class_dyn,
        &dyn_opts,
    );
    ResourceForm::new(ResourceKind::Service, SERVICE_TABS, nodes, None, service_on_change)
}

/// Build a service form with a dynamic list of class options and optional env defaults.
///
/// `options` — class IDs (e.g. ["proxy/zentinel", "proxy/traefik"]).
/// `default` — pre-selected class (should be the first/local entry).
/// `env_defaults` — raw "KEY=value\n..." string from `load_class_env_defaults()`.
pub fn new_service_form_with_class_options(
    options:         Vec<String>,
    default:         &str,
    env_defaults:    Option<&str>,
    project_slugs:   Vec<String>,
    host_slugs:      Vec<String>,
    current_project: &str,
    current_host:    &str,
) -> ResourceForm {
    let class_dyn  = [("class", default.to_string())];
    let env_str    = env_defaults.unwrap_or("").to_string();
    let mut prefill = HashMap::new();
    if !env_str.is_empty()         { prefill.insert("env", env_str.as_str()); }
    if !current_project.is_empty() { prefill.insert("project", current_project); }
    if !current_host.is_empty()    { prefill.insert("host", current_host); }
    let dyn_opts = vec![
        ("project", project_slugs),
        ("host",    host_slugs),
        ("class",   options),
    ];
    let nodes = schema_form::build_nodes(
        ServiceFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &class_dyn,
        &dyn_opts,
    );
    ResourceForm::new(ResourceKind::Service, SERVICE_TABS, nodes, None, service_on_change)
}

// ── Submit ────────────────────────────────────────────────────────────────────

/// Write a standalone `.service.toml` file for the service instance.
///
/// Project and host are read from form fields (required).
pub fn submit_service_form(form: &ResourceForm, services_dir: &Path) -> Result<()> {
    let name        = form.field_value("name");
    let class       = form.field_value("class");
    let project     = form.field_value("project");
    let host        = form.field_value("host");
    let version     = form.field_value("version");
    let tags_raw    = form.field_value("tags");
    let subdomain   = form.field_value("subdomain");
    let alias       = form.field_value("alias");
    let port        = form.field_value("port");
    let external    = form.field_value("external");
    let description = form.field_value("description");
    let git_repo    = form.field_value("git_repo");
    let website     = form.field_value("website");

    if name.is_empty()    { anyhow::bail!("Service name is required"); }
    if class.is_empty()   { anyhow::bail!("Service class is required"); }
    if project.is_empty() { anyhow::bail!("Project is required"); }
    if host.is_empty()    { anyhow::bail!("Host is required"); }

    let slug        = crate::app::slugify(&name);
    let version_val = if version.is_empty() { "latest".to_string() } else { version };
    let path        = services_dir.join(format!("{}.service.toml", slug));

    // Reject duplicate names — service names must be unique within a project.
    if path.exists() && form.edit_id.is_none() {
        anyhow::bail!("A service named '{}' already exists in this project", slug);
    }

    let mut content = format!(
        "[service]\nname          = \"{name}\"\nservice_class = \"{class}\"\nproject       = \"{project}\"\nhost          = \"{host}\"\nversion       = \"{version_val}\"\n"
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
    if external == "true" {
        content.push_str("external      = true\n");
    }
    if !description.is_empty() {
        let desc_escaped = crate::ui::widgets::toml_escape_str(&description);
        content.push_str(&format!("description   = \"{desc_escaped}\"\n"));
    }
    if !git_repo.is_empty() {
        content.push_str(&format!("git_repo      = \"{git_repo}\"\n"));
    }
    if !website.is_empty() {
        content.push_str(&format!("website       = \"{website}\"\n"));
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

    // Env vars: parse "KEY=value" lines and "# comment" lines from the env field.
    // Comments (# ...) attach to the NEXT key-value pair as metadata → [vars_comments].
    let env_raw = form.field_value("env");
    let mut env_pairs:    Vec<(String, String)> = Vec::new();
    let mut env_comments: Vec<(String, String)> = Vec::new();  // (key, comment)
    let mut pending_comment = String::new();
    for line in env_raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            pending_comment.clear();
            continue;
        }
        if let Some(comment) = line.strip_prefix('#') {
            pending_comment = comment.trim().to_string();
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let k = k.trim().to_string();
            if k.is_empty() { pending_comment.clear(); continue; }
            let v = crate::ui::widgets::toml_escape_str(v.trim());
            if !pending_comment.is_empty() {
                env_comments.push((k.clone(), std::mem::take(&mut pending_comment)));
            }
            env_pairs.push((k, v));
        }
        pending_comment.clear();
    }
    if !env_pairs.is_empty() {
        content.push_str("\n[vars]\n");
        for (k, v) in &env_pairs {
            content.push_str(&format!("{k} = \"{v}\"\n"));
        }
    }
    if !env_comments.is_empty() {
        content.push_str("\n[vars_comments]\n");
        for (k, c) in &env_comments {
            let c_escaped = crate::ui::widgets::toml_escape_str(c);
            content.push_str(&format!("{k} = \"{c_escaped}\"\n"));
        }
    }

    std::fs::write(&path, content)?;
    Ok(())
}

/// Build a pre-filled service form for editing an existing service instance.
///
/// `svc_entry` provides the project-TOML metadata (alias, host, subdomain, …).
/// `svc_config` is the standalone `.service.toml` parsed config — used to
/// populate the env table with the correct [vars] + [vars_comments] data.
/// When absent (file missing or unreadable) the form still opens with empty env.
pub fn edit_service_form(
    svc_name:        &str,
    svc_entry:       &fsn_core::config::project::ServiceEntry,
    svc_config:      Option<&fsn_core::config::project::ServiceInstanceConfig>,
    edit_slug:       String,
    project_slugs:   Vec<String>,
    host_slugs:      Vec<String>,
    current_project: &str,
) -> ResourceForm {
    let tags  = svc_entry.tags.join(", ");
    let port  = svc_entry.port.map(|p| p.to_string()).unwrap_or_default();
    let sub   = svc_entry.subdomain.as_deref().unwrap_or("");
    let alias = svc_entry.alias.as_deref().unwrap_or("");
    let ver   = svc_entry.version.as_str();

    // Read project/host/description/external/git_repo/website from standalone config if available.
    let project_val = svc_config.map(|c| c.service.project.as_str()).unwrap_or(current_project);
    let host_val    = svc_config.and_then(|c| c.service.host.as_deref()).or(svc_entry.host.as_deref()).unwrap_or("");
    let desc_val    = svc_config.and_then(|c| c.service.meta.description.as_deref()).unwrap_or("");
    let ext_val     = if svc_config.map(|c| c.service.external).unwrap_or(false) { "true" } else { "false" };
    let git_val     = svc_config.and_then(|c| c.service.git_repo.as_deref()).unwrap_or("");
    let web_val     = svc_config.and_then(|c| c.service.website.as_deref()).unwrap_or("");

    // Build env string from standalone file (vars + vars_comments).
    // Format: "# comment\nKEY=value\n..." — EnvTableNode parses this correctly.
    let env_str: String = if let Some(cfg) = svc_config {
        cfg.vars.iter()
            .map(|(k, v)| {
                let val = match v {
                    toml::Value::String(s) => s.clone(),
                    other                  => other.to_string(),
                };
                if let Some(comment) = cfg.vars_comments.get(k) {
                    format!("# {comment}\n{k}={val}")
                } else {
                    format!("{k}={val}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        // Fallback: project-TOML env (no comments)
        svc_entry.env.iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mut prefill = HashMap::new();
    prefill.insert("name",        svc_name);
    prefill.insert("class",       svc_entry.service_class.as_str());
    prefill.insert("project",     project_val);
    prefill.insert("host",        host_val);
    prefill.insert("version",     ver);
    prefill.insert("tags",        tags.as_str());
    prefill.insert("subdomain",   sub);
    prefill.insert("alias",       alias);
    prefill.insert("port",        port.as_str());
    prefill.insert("external",    ext_val);
    prefill.insert("description", desc_val);
    prefill.insert("git_repo",    git_val);
    prefill.insert("website",     web_val);
    prefill.insert("env",         env_str.as_str());

    let dyn_opts = vec![
        ("project", project_slugs),
        ("host",    host_slugs),
    ];
    let nodes = schema_form::build_nodes(
        ServiceFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &[],
        &dyn_opts,
    );
    ResourceForm::new(ResourceKind::Service, SERVICE_TABS, nodes, Some(edit_slug), service_on_change)
}
