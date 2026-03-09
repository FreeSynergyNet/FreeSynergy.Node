// Host-specific form — uses #[derive(Form)] for schema definition.
//
// Tabs:
//   Tab 0 (Host)   : name, alias, address, project (dynamic dropdown)
//   Tab 1 (System) : ssh_user, ssh_port, install_dir
//   Tab 2 (DNS/TLS): dns_provider, acme_provider, acme_email

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use fsn_form::Form;

use crate::app::{HostHandle, ResourceForm, ResourceKind, HOST_TABS};
use crate::schema_form;
use crate::ui::form_node::FormNode;

// ── Form data struct ──────────────────────────────────────────────────────────

/// Form schema for creating and editing a Host.
#[derive(Form)]
pub struct HostFormData {
    // ── Tab 0: Host ───────────────────────────────────────────────────────
    #[form(label = "form.host.name", required, tab = 0, hint = "form.host.name.hint")]
    pub name: String,

    #[form(label = "form.host.alias", tab = 0, hint = "form.host.alias.hint")]
    pub alias: String,

    #[form(label = "form.host.address", required, tab = 0, hint = "form.host.address.hint")]
    pub address: String,

    /// Project this host belongs to.
    /// Options are populated dynamically at form-build time (project slugs).
    #[form(label = "form.host.project", widget = "select", tab = 0)]
    pub project: String,

    // ── Tab 1: System ─────────────────────────────────────────────────────
    #[form(label = "form.host.ssh_user", tab = 1, default = "root")]
    pub ssh_user: String,

    #[form(label = "form.host.ssh_port", tab = 1, default = "22")]
    pub ssh_port: String,

    #[form(label = "form.host.install_dir", tab = 1, hint = "form.host.install_dir.hint", default = "/opt/fsn")]
    pub install_dir: String,

    // ── Tab 2: DNS / TLS ──────────────────────────────────────────────────
    #[form(label = "form.host.dns_provider", widget = "select", tab = 2,
           options = "hetzner,cloudflare,manual,none", default = "hetzner")]
    pub dns_provider: String,

    #[form(label = "form.host.acme_provider", widget = "select", tab = 2,
           options = "letsencrypt,zerossl,buypass,none", default = "letsencrypt")]
    pub acme_provider: String,

    #[form(label = "form.host.acme_email", tab = 2, widget = "email", hint = "form.host.acme_email.hint")]
    pub acme_email: String,
}

// ── Display helpers ───────────────────────────────────────────────────────────

pub fn dns_provider_display(code: &str) -> &'static str {
    match code {
        "hetzner"    => "Hetzner DNS",
        "cloudflare" => "Cloudflare",
        "manual"     => "Manual",
        "none"       => "None (disabled)",
        _            => "—",
    }
}

pub fn acme_provider_display(code: &str) -> &'static str {
    match code {
        "letsencrypt" => "Let's Encrypt",
        "zerossl"     => "ZeroSSL",
        "buypass"     => "Buypass",
        "none"        => "None (disabled)",
        _             => "—",
    }
}

const DISPLAY_FNS: &[(&str, fn(&str) -> &'static str)] = &[
    ("dns_provider",  dns_provider_display),
    ("acme_provider", acme_provider_display),
];

// ── Smart-defaults hook ───────────────────────────────────────────────────────

fn host_on_change(nodes: &mut Vec<Box<dyn FormNode>>, key: &'static str) {
    if key == "address" {
        let addr = nodes.iter().find(|n| n.key() == "address")
            .map(|n| n.value().to_string()).unwrap_or_default();
        // Auto-derive acme_email only from FQDN (not raw IPs)
        if addr.contains('.') && !addr.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            let acme_dirty = nodes.iter().find(|n| n.key() == "acme_email")
                .map(|n| n.is_dirty()).unwrap_or(false);
            if !acme_dirty {
                if let Some(n) = nodes.iter_mut().find(|n| n.key() == "acme_email") {
                    n.set_value(&format!("admin@{}", addr));
                }
            }
        }
    }
}

// ── Form builders ─────────────────────────────────────────────────────────────

/// Build a "New Host" form.
///
/// `project_slugs`   — Available projects for the dropdown.
/// `current_project` — Pre-selected project slug (usually the active project).
pub fn new_host_form(project_slugs: Vec<String>, current_project: &str) -> ResourceForm {
    let mut prefill = HashMap::new();
    if !current_project.is_empty() {
        prefill.insert("project", current_project);
    }
    let dyn_opts = [("project", project_slugs)];
    let nodes = schema_form::build_nodes(
        HostFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &[],
        &dyn_opts,
    );
    ResourceForm::new(ResourceKind::Host, HOST_TABS, nodes, None, host_on_change)
}

pub fn edit_host_form(handle: &HostHandle, project_slugs: Vec<String>) -> ResourceForm {
    let h = &handle.config.host;
    let ssh_port_str = h.ssh_port.to_string();
    let prefill: HashMap<&str, &str> = [
        ("name",        h.name.as_str()),
        ("alias",       h.alias.as_deref().unwrap_or("")),
        ("address",     h.addr()),
        ("project",     h.project.as_deref().unwrap_or("")),
        ("ssh_user",    h.ssh_user.as_str()),
        ("ssh_port",    ssh_port_str.as_str()),
        ("install_dir", h.install_dir.as_deref().unwrap_or("")),
    ].into_iter().filter(|(_, v)| !v.is_empty()).collect();

    let dyn_opts = [("project", project_slugs)];
    let nodes = schema_form::build_nodes(
        HostFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &[],
        &dyn_opts,
    );
    ResourceForm::new(ResourceKind::Host, HOST_TABS, nodes, Some(handle.slug.clone()), host_on_change)
}

// ── Submit ────────────────────────────────────────────────────────────────────

pub fn submit_host_form(form: &ResourceForm, project_dir: &Path) -> Result<()> {
    let name    = form.field_value("name");
    let alias   = form.field_value("alias");
    let address = form.field_value("address");
    let project = form.field_value("project");

    if name.is_empty()    { anyhow::bail!("Hostname ist erforderlich"); }
    if address.is_empty() { anyhow::bail!("IP-Adresse / FQDN ist erforderlich"); }

    let ssh_user    = form.field_value("ssh_user");
    let ssh_port    = form.field_value("ssh_port");
    let install_dir = form.field_value("install_dir");
    let dns_prov    = form.field_value("dns_provider");
    let acme_prov   = form.field_value("acme_provider");
    let acme_email  = form.field_value("acme_email");

    let ssh_user_val      = if ssh_user.is_empty()    { "root".to_string()     } else { ssh_user };
    let ssh_port_val: u16 = ssh_port.parse().unwrap_or(22);
    let install_dir_val   = if install_dir.is_empty() { "/opt/fsn".to_string() } else { install_dir };

    let slug = crate::app::slugify(&name);
    let path = project_dir.join(format!("{}.host.toml", slug));

    let mut content = format!(
        "[host]\nname        = \"{name}\"\naddress     = \"{address}\"\n"
    );
    if !alias.is_empty()   { content.push_str(&format!("alias       = \"{alias}\"\n")); }
    if !project.is_empty() { content.push_str(&format!("project     = \"{project}\"\n")); }
    content.push_str(&format!(
        "ssh_user    = \"{ssh_user_val}\"\nssh_port    = {ssh_port_val}\ninstall_dir = \"{install_dir_val}\"\n"
    ));

    if dns_prov != "none" && !dns_prov.is_empty() {
        content.push_str(&format!("\n[dns]\nprovider = \"{dns_prov}\"\nzones    = []\n"));
    }
    if acme_prov != "none" && !acme_prov.is_empty() {
        let email = if acme_email.is_empty() { format!("admin@{}", address) } else { acme_email };
        content.push_str(&format!("\n[acme]\nemail    = \"{email}\"\nprovider = \"{acme_prov}\"\n"));
    }

    std::fs::write(&path, content)?;
    Ok(())
}
