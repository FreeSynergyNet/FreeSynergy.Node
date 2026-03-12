// i18n — translation system.
//
// Design Pattern: Strategy — Lang selects which translation table to use.
// English is the only compile-time built-in and the universal fallback.
// All other languages (including German) are loaded from TOML files at runtime.
//
// Key convention: "section.key" e.g. "welcome.title", "status.running"
// All keys and built-in strings are &'static str.
//
// DynamicLang uses Box::leak() so that loaded strings are also &'static str —
// this keeps the return type of t() uniform and the Lang enum Copy.

use std::collections::HashMap;
use crate::app::Lang;

/// Translation API version.
/// Increment when keys are added or renamed.
/// Language files with a different api_version are shown as potentially stale.
pub const TRANSLATION_API_VERSION: u32 = 1;

// ── DynamicLang ───────────────────────────────────────────────────────────────

/// A language loaded at runtime from a TOML file.
///
/// All strings are leaked so they are `'static`, matching the static English
/// strings and keeping `Lang` (which holds `&'static DynamicLang`) `Copy`.
#[derive(Debug, PartialEq, Eq)]
pub struct DynamicLang {
    pub code:         &'static str,   // e.g. "de"
    pub code_upper:   &'static str,   // e.g. "DE"
    pub name:         &'static str,   // e.g. "Deutsch"
    pub api_version:  u32,
    pub completeness: u8,
    map:              HashMap<&'static str, &'static str>,
    field_map:        HashMap<&'static str, &'static str>,
}

impl DynamicLang {
    /// Parse TOML source and leak the result as `&'static DynamicLang`.
    ///
    /// Memory is intentionally leaked — translations live for the entire app
    /// lifetime (a few hundred KB total), so leaking is the right trade-off.
    pub fn load(content: &str) -> anyhow::Result<&'static Self> {
        let doc: toml::Value = toml::from_str(content)
            .map_err(|e| anyhow::anyhow!("TOML parse error: {e}"))?;

        let meta = doc.get("meta")
            .ok_or_else(|| anyhow::anyhow!("missing [meta] section"))?;

        let code = meta.get("language").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing meta.language"))?.to_owned();
        let name = meta.get("name").and_then(|v| v.as_str())
            .unwrap_or(&code).to_owned();
        let api_version = meta.get("api_version")
            .and_then(|v| v.as_integer()).unwrap_or(1) as u32;
        let completeness = meta.get("completeness")
            .and_then(|v| v.as_integer()).unwrap_or(0).clamp(0, 100) as u8;

        let mut map: HashMap<&'static str, &'static str> = HashMap::new();
        if let Some(table) = doc.get("keys").and_then(|v| v.as_table()) {
            for (k, v) in table {
                if let Some(val) = v.as_str() {
                    let k: &'static str = Box::leak(k.clone().into_boxed_str());
                    let v: &'static str = Box::leak(val.to_owned().into_boxed_str());
                    map.insert(k, v);
                }
            }
        }

        let mut field_map: HashMap<&'static str, &'static str> = HashMap::new();
        if let Some(table) = doc.get("field_help").and_then(|v| v.as_table()) {
            for (k, v) in table {
                if let Some(val) = v.as_str() {
                    let k: &'static str = Box::leak(k.clone().into_boxed_str());
                    let v: &'static str = Box::leak(val.to_owned().into_boxed_str());
                    field_map.insert(k, v);
                }
            }
        }

        let code_upper: &'static str = Box::leak(code.to_uppercase().into_boxed_str());
        let lang = DynamicLang {
            code:         Box::leak(code.into_boxed_str()),
            code_upper,
            name:         Box::leak(name.into_boxed_str()),
            api_version,
            completeness,
            map,
            field_map,
        };
        Ok(Box::leak(Box::new(lang)))
    }

    /// Load all `.toml` files from `dir`, silently skipping failures.
    pub fn load_dir(dir: &std::path::Path) -> Vec<&'static Self> {
        let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
        let mut out = Vec::new();
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") { continue; }
            let Ok(content) = std::fs::read_to_string(&path) else { continue; };
            match DynamicLang::load(&content) {
                Ok(lang) => out.push(lang),
                Err(e)   => tracing::warn!("i18n: failed to load {:?}: {e}", path),
            }
        }
        out
    }
}

// ── Translate trait ───────────────────────────────────────────────────────────

/// Translation context — abstract over the concrete language.
///
/// Analogous to React's `t()` from i18next. Components take `&impl Translate`
/// instead of `&AppState`, keeping them reusable across contexts.
pub trait Translate {
    fn t(&self, key: &'static str) -> &'static str;
}

impl Translate for Lang {
    fn t(&self, key: &'static str) -> &'static str {
        match self {
            Lang::En         => en(key).unwrap_or(key),
            Lang::Dynamic(d) => d.map.get(key).copied()
                                    .unwrap_or_else(|| en(key).unwrap_or(key)),
        }
    }
}

// ── Free translation functions ────────────────────────────────────────────────

/// Translate `key` for the given `lang`. Falls back to English, then to `key`.
///
/// `key` may be any `&str` lifetime; callers typically pass `&'static str`
/// literals, in which case the return is also effectively `'static`.
pub fn t<'a>(lang: Lang, key: &'a str) -> &'a str {
    match lang {
        Lang::En         => en(key).unwrap_or(key),
        Lang::Dynamic(d) => d.map.get(key).copied()
                                .unwrap_or_else(|| en(key).unwrap_or(key)),
    }
}

/// Contextual field help string for the F1 sidebar. Falls back to English.
pub fn field_help(lang: Lang, field_key: &str) -> Option<&'static str> {
    match lang {
        Lang::En         => en_field_help(field_key),
        Lang::Dynamic(d) => d.field_map.get(field_key).copied()
                                .or_else(|| en_field_help(field_key)),
    }
}

// ── English (built-in, always available) ─────────────────────────────────────

fn en(key: &str) -> Option<&'static str> {
    Some(match key {
        "welcome.title"         => "Welcome to FreeSynergy.Node",
        "welcome.subtitle"      => "Decentralized infrastructure — free and self-hosted",
        "welcome.new_project"   => "New Project",
        "welcome.open_project"  => "Open Project",
        "welcome.open_disabled" => "(coming soon)",
        "welcome.hint"          => "←→=Select  Enter=Confirm  L=Language  q=Quit",
        "sys.host"    => "Host",
        "sys.user"    => "User",
        "sys.ip"      => "IP Address",
        "sys.ram"     => "Memory",
        "sys.cpu"     => "CPU Cores",
        "sys.uptime"  => "Uptime",
        "sys.podman"  => "Podman",
        "sys.arch"    => "Architecture",
        "lang.label"  => "Language",
        "lang.de"     => "Deutsch",
        "lang.en"     => "English",
        "dash.services"   => "Services",
        "dash.col.name"   => "Name",
        "dash.col.type"   => "Type",
        "dash.col.domain" => "Domain",
        "dash.col.status" => "Status",
        "dash.hint"           => "↑↓=Nav  /=Search  y=Copy  n=New  e=Edit  x=Delete  Tab=Services  q=Quit",
        "dash.hint.host"      => "↑↓=Nav  n=New Host  e=Edit  s=Start  x=Delete  Tab=Detail  q=Quit",
        "dash.hint.service"   => "↑↓=Nav  n=New Service  e=Edit  s=Start  x=Delete  Tab=Detail  q=Quit",
        "dash.hint.services"  => "↑↓=Nav  Space=Select  s=Start  r=Restart  x=Stop  d=Deploy  l=Logs  y=Copy  q=Quit",
        "dash.hint.confirm"   => "Really delete project?  Y=Yes  N=Cancel",
        "dash.no_projects"    => "(No project found)",
        "dash.no_services"         => "(No services configured)",
        "dash.no_project_selected" => "No project selected",
        "dash.hint.f1"             => "F1=Help",
        "dash.hint.quit"           => "q=Quit",
        "dash.tab.projects"      => "Projects",
        "dash.tab.hosts"         => "Hosts",
        "dash.tab.services"      => "Services",
        "dash.tab.store"         => "Store",
        "dash.tab.settings"      => "⚙ Settings",
        "dash.new_project"    => "+ New Project",
        "dash.new_service"    => "+ New Service",
        "welcome.edit_project" => "Edit Project",
        "form.submit.edit"    => "Save",
        "sidebar.projects"    => "Projects",
        "sidebar.hosts"       => "Hosts",
        "sidebar.services"    => "Services",
        "sidebar.system"  => "System",
        "status.running"  => "● Running",
        "status.stopped"  => "○ Stopped",
        "status.error"    => "✗ Error",
        "status.unknown"  => "? Unknown",
        "logs.hint"            => "q=Close  ↑↓=Scroll",
        "form.tab.project"     => "Project",
        "form.tab.options"     => "Options",
        "form.textarea.hint"   => "Tab=Next field  Enter=New line  Alt+Enter=Submit  Esc=Back",
        "form.confirm.leave"   => "Discard changes and close?  Y=Yes  other key=No",
        "confirm.quit"              => "Really quit?  Y=Yes  other key=No",
        "confirm.delete.project"    => "Really delete project?  Y=Yes  other key=No",
        "confirm.delete.service"    => "Delete service?  Y=Yes  other key=No",
        "confirm.delete.host"       => "Really delete host?  Y=Yes  other key=No",
        "confirm.stop.service"      => "Stop service?  Y=Yes  other key=No",
        "form.hint.ctrl"       => "^←=Prev Tab  ^→=Next Tab  ^C=Quit",
        "form.required"             => "* Required",
        "form.all_required_filled"  => "✓ All required fields filled",
        "form.missing_required"     => "required field(s) still empty",
        "form.multiselect.none"     => "(Nothing selected)",
        "form.error"           => "Error",
        "form.submit"          => "Create Project",
        "form.submit.service"  => "Create Service",
        "form.submit.host"     => "Create Host",
        "form.project.name"              => "Project Name",
        "form.project.name.hint"         => "Short name without spaces, e.g. myproject",
        "form.project.domain"            => "Domain",
        "form.project.domain.hint"       => "Primary domain, e.g. example.com  (derived from project name)",
        "form.project.description"       => "Description",
        "form.project.description.hint"  => "Short project description (optional)",
        "form.project.path"              => "Install Directory",
        "form.project.path.hint"         => "Where fsn stores data (created if it does not exist)",
        "form.project.email"             => "Contact Email",
        "form.project.email.hint"        => "For Let's Encrypt notifications (derived from domain)",
        "form.options.language"    => "Primary Language (↑↓ to select)",
        "form.options.version"     => "Version",
        "form.tab.service"         => "Service",
        "form.tab.network"         => "Network",
        "form.tab.env"             => "Environment",
        "form.new_service"         => "New Service",
        "form.edit_service"        => "Edit Service",
        "form.service.name"        => "Service Name",
        "form.service.name.hint"   => "Instance name, e.g. forgejo (unique within project)",
        "form.service.class"       => "Service Type (↑↓ to select)",
        "form.service.tags"        => "Tags (comma-separated)",
        "form.service.tags.hint"   => "Optional tags, e.g. internal,critical",
        "form.service.alias"           => "Subdomain Alias",
        "form.service.alias.hint"      => "Optional alias, e.g. git → git.<domain>",
        "form.service.subdomain"       => "Subdomain",
        "form.service.subdomain.hint"  => "Subdomain for this instance (derived from name)",
        "form.service.port"            => "Port (optional)",
        "form.service.env"             => "Environment Variables",
        "form.service.env.hint"        => "↑↓=row  Tab=column  Enter=new row  ↓=new row at end",
        "form.tab.host"     => "Host",
        "form.tab.system"   => "System",
        "form.tab.dns"      => "DNS / TLS",
        "form.new_host"     => "New Host",
        "form.edit_host"    => "Edit Host",
        "form.host.name"            => "Hostname",
        "form.host.name.hint"       => "Unique name, e.g. server1 (no spaces)",
        "form.host.alias"           => "Alias",
        "form.host.alias.hint"      => "Display name, e.g. main or backup",
        "form.host.address"         => "IP Address / FQDN",
        "form.host.address.hint"    => "Primary IPv4 address or FQDN, e.g. 192.168.1.1",
        "form.host.project"         => "Project",
        "form.host.ssh_user"        => "SSH User",
        "form.host.ssh_port"        => "SSH Port",
        "form.host.install_dir"     => "Install Directory",
        "form.host.install_dir.hint" => "Base directory on this host, e.g. /opt/fsn",
        "form.host.dns_provider"    => "DNS Provider (↑↓ to select)",
        "form.host.acme_provider"   => "ACME Provider (↑↓ to select)",
        "form.host.acme_email"      => "ACME Email",
        "form.host.acme_email.hint" => "Contact email for Let's Encrypt (derived from address)",
        "dash.new_host"   => "+ New Host",
        "deploy.title"    => "Compose Export",
        "deploy.hint"     => "q=Close",
        "deploy.running"  => "↻ Running...",
        "form.tab.bot"           => "Bot",
        "form.submit.bot"        => "Create Bot",
        "form.bot.name"          => "Bot Name",
        "form.bot.name.hint"     => "Unique name, e.g. matrix-bot (no spaces)",
        "form.bot.type"          => "Bot Type (↑↓ to select)",
        "form.bot.class"         => "Bot Class (↑↓ to select)",
        "form.bot.description"   => "Description",
        "form.bot.tags"          => "Tags",
        "form.bot.tags.hint"     => "Comma-separated tags, e.g. notifications,alerts",
        // ── Wizard ───────────────────────────────────────────────────────
        "wizard.title"        => "Wizard",
        "wizard.hint"         => "Tab=Next Field  ^Enter=Save  Esc=Cancel",
        "task.new_project"    => "Project",
        "task.new_host"       => "Host",
        "task.new_proxy"      => "Proxy",
        "task.new_iam"        => "IAM",
        "task.new_mail"       => "Mail",
        "task.new_service"    => "Service",
        // ── New-resource selector ─────────────────────────────────────────
        "new.resource.title"  => "Create New",
        "new.project"         => "New Project",
        "new.host"            => "New Host",
        "new.service"         => "New Service",
        "new.bot"             => "New Bot",
        "new.resource.hint"   => "↑↓=Select  Enter=Open  Esc=Cancel",
        // ── Help sidebar ──────────────────────────────────────────────────
        "help.title"          => "Help (F1)",
        "help.close_hint"     => "F1=Close",
        "help.nav"            => "Navigation",
        "help.nav.select"     => "Select row",
        "help.nav.open"       => "Open / Confirm",
        "help.nav.panel"      => "Switch panel",
        "help.new_project"    => "Create new project",
        "help.lang"           => "Toggle language",
        "help.quit"           => "Quit",
        "help.deploy"         => "Deploy project",
        "help.export"         => "Export Compose file",
        "help.delete"         => "Delete",
        "help.form.project"   => "Project Form",
        "help.form.host"      => "Host Form",
        "help.form.service"   => "Service Form",
        "help.form.bot"       => "Bot Form",
        "help.form.next"      => "Next field",
        "help.form.prev"      => "Previous field",
        "help.form.select"    => "Choose option",
        "help.form.advance"   => "Next tab",
        "help.form.submit"    => "Submit form",
        "help.form.tab_next"  => "Tab forward",
        "help.form.tab_prev"  => "Tab backward",
        "help.form.cancel"    => "Cancel / Close",
        "help.field"          => "This field",
        "help.settings"       => "Settings",
        // ── Settings screen ───────────────────────────────────────────────
        "settings.title"         => "Settings – Module Stores",
        "settings.hint"          => "A=Add  D=Delete  Space=Enable/Disable  Esc=Back",
        "settings.stores.header" => "Configured Stores",
        "settings.store.enabled" => "✓ enabled",
        "settings.store.disabled"=> "✗ disabled",
        "settings.store.add.prompt" => "Store URL (e.g. https://github.com/you/modules):",
        "settings.store.name.prompt"=> "Store display name:",
        "settings.empty"         => "(no stores configured)",
        // ── Sidebar filter ────────────────────────────────────────────────
        "dash.filter.empty"      => "(no matches)",
        "dash.hint.filter"       => "Esc=Close  ↑↓=Nav  Enter=Select  Type=Search",
        // ── Multi-select ──────────────────────────────────────────────────
        "dash.hint.multiselect"  => "Space=Select  s=Start all  x=Stop all  u=Deselect",
        // ── Form navigation ───────────────────────────────────────────────
        "form.hint"              => "Enter=Next field  Tab=Switch tab  ↑↓=Select  ^S=Submit  Esc=Close",
        // ── Project form – slot / tag fields ─────────────────────────────
        "form.tab.services"            => "Services",
        "form.project.tags"            => "Tags (comma-separated)",
        "form.project.tags.hint"       => "Optional tags, e.g. production,internal",
        "form.project.iam"             => "IAM service (instance name)",
        "form.project.iam.hint"        => "Instance name, e.g. kanidm",
        "form.project.wiki"            => "Wiki service (instance name)",
        "form.project.wiki.hint"       => "Instance name, e.g. outline",
        "form.project.mail"            => "Mail service (instance name)",
        "form.project.mail.hint"       => "Instance name, e.g. stalwart",
        "form.project.monitoring"      => "Monitoring (instance name)",
        "form.project.monitoring.hint" => "Instance name, e.g. netdata",
        "form.project.git"             => "Git service (instance name)",
        "form.project.git.hint"        => "Instance name, e.g. forgejo",
        // ── Host form – proxy field ───────────────────────────────────────
        "form.host.proxy"              => "Proxy instance",
        "form.host.proxy.hint"         => "Zentinel instance name on this host (default: zentinel)",
        // ── Context menu ─────────────────────────────────────────────────
        "ctx.edit"   => "Edit",
        "ctx.delete" => "Delete",
        "ctx.deploy" => "Deploy",
        "ctx.start"  => "Start",
        "ctx.stop"   => "Stop",
        "ctx.logs"   => "View Logs",
        // ── Selection popup hints ─────────────────────────────────────────
        "popup.hint.single" => "↑↓=Navigate  Enter=OK  Esc=Cancel",
        "popup.hint.multi"  => "↑↓=Navigate  Space=Toggle  Enter=OK  Esc=Cancel",
        // ── Settings tabs ─────────────────────────────────────────────────
        "settings.tab.stores"    => "Stores",
        "settings.tab.languages" => "Languages",
        // ── Settings – Language tab ───────────────────────────────────────
        "settings.lang.title"    => "Available Languages",
        "settings.lang.builtin"  => "(built-in)",
        "settings.lang.active"   => "● active",
        "settings.lang.inactive" => "  inactive",
        "settings.lang.complete" => "complete",
        "settings.lang.api_ok"   => "up to date",
        "settings.lang.api_warn" => "⚠ version mismatch",
        "settings.lang.none"     => "(no languages installed — download from Store)",
        "settings.lang.hint"     => "↑↓=Navigate  Enter=Activate  Del=Remove  Tab=Switch tab  Esc=Back",
        "settings.stores.hint"   => "A=Add  D=Delete  Space=Enable/Disable  Tab=Switch tab  Esc=Back",
        // ── Settings sections (sidebar labels) ────────────────────────────────
        "settings.section.stores"    => "Stores",
        "settings.section.store"     => "Store",
        "settings.section.languages" => "Languages",
        "settings.section.general"   => "General",
        "settings.section.about"     => "About",
        // ── Settings hints (context-sensitive) ────────────────────────────────
        "settings.hint.sidebar"    => "↑↓=Navigate  Enter/→=Open  Esc=Back",
        "settings.hint.stores"     => "↑↓=Nav  Enter=Edit  Space=Toggle  A=Add  D=Delete  ←=Sidebar",
        "settings.hint.languages"  => "↑↓=Nav  Enter=Activate  Space=Toggle(↓/✕)  Del=Remove  ←=Sidebar",
        "settings.hint.general"    => "←=Sidebar",
        "settings.hint.about"      => "←=Sidebar",
        // ── Settings → Store section ───────────────────────────────────────────
        "settings.store.hint.apply"  => "Ctrl+A: Apply changes",
        "settings.store.pending"     => "pending changes",
        "settings.store.tab.repos"   => "Repositories",
        "settings.store.tab.modules" => "Modules",
        // ── Store screen ──────────────────────────────────────────────────────
        "screen.store.title"         => "Store",
        "store.status.installed"     => "✓ Installed",
        "store.status.not_installed" => "Not installed",
        "store.status.update_available" => "↑ Update available",
        "store.hint.install"         => "[i] Install",
        "store.hint.uninstall"       => "[u] Uninstall",
        "store.hint.reinstall"       => "[r] Reinstall",
        "store.no_packages"          => "No packages in this category",
        "store.select_package"       => "Select a package to see details",
        "store.select_type"          => "Select a category to see details",
        "store.confirm.install"      => "Install module",
        "store.confirm.uninstall"    => "Uninstall module",
        "store.confirm.reinstall"    => "Reinstall module",
        _ => return None,
    })
}

// ── English field help (built-in) ─────────────────────────────────────────────

fn en_field_help(key: &str) -> Option<&'static str> {
    Some(match key {
        "name" =>
            "Unique short name. Lowercase letters, digits and hyphens only. Used for filenames and container names.",
        "domain" =>
            "Primary domain without http://. Wildcards allowed: *.example.com. Subdomains are derived automatically.",
        "description" =>
            "Optional description. Multi-line input supported. Shown in the project overview.",
        "email" | "acme_email" =>
            "Email address for Let's Encrypt (ACME). Used for certificate expiry notifications.",
        "address" =>
            "IPv4 address or FQDN of the host. Example: 192.168.1.10 or server.example.com",
        "subdomain" =>
            "Subdomain for this instance. Derived automatically from name. Results in: subdomain.domain.tld",
        "install_dir" =>
            "Base directory for data and config. Created if it does not exist.",
        "ssh_port" =>
            "Default is 22. Alternative ports for security, e.g. 2222.",
        _ => return None,
    })
}
