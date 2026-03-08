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
#[derive(Form)]
pub struct ServiceFormData {
    // ── Tab 0: Service ────────────────────────────────────────────────────
    #[form(label = "form.service.name", required, tab = 0, hint = "form.service.name.hint")]
    pub name: String,

    #[form(label = "form.service.class", widget = "select", required, tab = 0,
           options = "proxy/zentinel,git/forgejo,iam/kanidm,mail/stalwart,wiki/outline,chat/matrix,tasks/vikunja,monitoring/netdata",
           default = "proxy/zentinel")]
    pub class: String,

    #[form(label = "form.options.version", tab = 0, default = "latest")]
    pub version: String,

    #[form(label = "form.service.tags", tab = 0, hint = "form.service.tags.hint")]
    pub tags: String,

    // ── Tab 1: Network ────────────────────────────────────────────────────
    #[form(label = "form.service.subdomain", tab = 1, hint = "form.service.subdomain.hint")]
    pub subdomain: String,

    #[form(label = "form.service.alias", tab = 1, hint = "form.service.alias.hint")]
    pub alias: String,

    #[form(label = "form.service.port", tab = 1)]
    pub port: String,

    // ── Tab 2: Env ────────────────────────────────────────────────────────
    #[form(label = "form.service.env", widget = "env_table", tab = 2, rows = 4,
           hint = "form.service.env.hint")]
    pub env: String,
}

// ── Display helpers ───────────────────────────────────────────────────────────

pub fn service_class_display(code: &str) -> &'static str {
    match code {
        "proxy/zentinel"     => "Zentinel (Proxy)",
        "git/forgejo"        => "Forgejo (Git)",
        "iam/kanidm"         => "Kanidm (IAM)",
        "mail/stalwart"      => "Stalwart (Mail)",
        "wiki/outline"       => "Outline (Wiki)",
        "chat/matrix"        => "Matrix (Chat)",
        "tasks/vikunja"      => "Vikunja (Tasks)",
        "monitoring/netdata" => "Netdata (Monitoring)",
        _                    => "—",
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
}

// ── Form builder ──────────────────────────────────────────────────────────────

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

    std::fs::write(&path, content)?;
    Ok(())
}
