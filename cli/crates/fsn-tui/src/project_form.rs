// Project-specific form — uses #[derive(Form)] to declare the schema once.
//
// The schema drives form generation automatically:
//   ProjectFormData::schema() → FormSchema (static, generated at compile time)
//   schema_form::build_nodes(schema, prefill, display_fns, dynamics) → Vec<Box<dyn FormNode>>
//
// To add a new field: add it here with #[form(...)] attributes.
// No changes needed in events.rs, new_project.rs, or anywhere else.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use fsn_form::Form;

use fsn_core::store::StoreEntry;

use crate::app::{ProjectHandle, ResourceForm, ResourceKind, PROJECT_TABS, ServiceHandle};
use crate::schema_form;
use crate::ui::form_node::FormNode;

// ── Form data struct ──────────────────────────────────────────────────────────

/// Form schema for creating and editing a Project.
///
/// Each field maps to a `FieldMeta` in the generated `FormSchema`.
/// The actual domain struct (`ProjectMeta`) stays clean — no UI concerns.
///
/// Layout uses a 12-column grid — `col` declares the column span.
/// `min_w` sets the minimum rendered width; if the terminal is too narrow the
/// field wraps to its own row automatically.
#[derive(Form)]
pub struct ProjectFormData {
    // ── Section: Basis ────────────────────────────────────────────────────
    #[form(widget = "section", label = "form.section.basis", tab = 0)]
    pub _section_basis: String,

    // Name (col=7) + Domain (col=5) side-by-side; min_w=28 each so they wrap on tiny terminals.
    #[form(label = "form.project.name", required, tab = 0, col = 7, min_w = 28,
           hint = "form.project.name.hint", max_len = 255)]
    pub name: String,

    #[form(label = "form.project.domain", required, tab = 0, col = 5, min_w = 22,
           hint = "form.project.domain.hint")]
    pub domain: String,

    // Alias (col=4) + Version (col=3) + UI Language (col=5) in one row.
    #[form(label = "form.project.alias", tab = 0, col = 4, min_w = 18,
           hint = "form.project.alias.hint")]
    pub alias: String,

    #[form(label = "form.options.version", tab = 0, col = 3, min_w = 14, default = "0.1.0")]
    pub version: String,

    #[form(label = "form.options.language", widget = "select", tab = 0, col = 4, min_w = 18,
           options = "de,en,fr,es,it,pt", default = "de")]
    pub language: String,

    #[form(label = "form.project.path", required, tab = 0, col = 5, min_w = 22,
           widget = "dir_picker", hint = "form.project.path.hint")]
    pub install_dir: String,

    // ── Section: Details ──────────────────────────────────────────────────
    #[form(widget = "section", label = "form.section.details", tab = 0)]
    pub _section_details: String,

    #[form(label = "form.project.description", widget = "textarea", rows = 3, tab = 0,
           hint = "form.project.description.hint")]
    pub description: String,

    // Email (col=8) + Tags (col=4) side-by-side.
    #[form(label = "form.project.email", required, tab = 0, col = 8, min_w = 30,
           widget = "email", hint = "form.project.email.hint")]
    pub contact_email: String,

    #[form(label = "form.project.tags", tab = 0, col = 4, min_w = 18,
           hint = "form.project.tags.hint")]
    pub tags: String,

    // ── Section: Sprachen ─────────────────────────────────────────────────
    #[form(widget = "section", label = "form.section.languages", tab = 0)]
    pub _section_languages: String,

    /// Languages the project content is available in (multi-select).
    /// Options are populated dynamically from installed store languages — see build_lang_options().
    #[form(label = "form.project.languages", widget = "multi_select", tab = 0,
           hint = "form.project.languages.hint",
           options = "en")]
    pub languages: String,

    // ── Section: Services ─────────────────────────────────────────────────
    #[form(widget = "section", label = "form.section.services", tab = 0)]
    pub _section_services: String,
    // Service slot nodes (ServiceSlotNode) are appended programmatically
    // by append_slot_nodes() after build_nodes() — not declared in the schema here.
}

// ── Display helpers ───────────────────────────────────────────────────────────

pub fn lang_display(code: &str) -> String {
    match code {
        "de" => "Deutsch",
        "en" => "English",
        "fr" => "Français",
        "es" => "Español",
        "it" => "Italiano",
        "pt" => "Português",
        "ru" => "Русский",
        "zh" => "中文",
        "ja" => "日本語",
        "ar" => "العربية",
        _    => return code.to_string(),
    }.to_string()
}

const DISPLAY_FNS: &[(&str, fn(&str) -> String)] = &[
    // "language" (single select for UI lang) → native name display.
    // "languages" (multi-select for project content langs) → codes shown directly
    // because the options are dynamic and cannot be resolved by a static fn.
    ("language", lang_display),
];

/// Build the SlotEntry list for a single service slot field.
///
/// Populates (in order):
///   Configured — local service instances whose class starts with `class_prefix`
///   Available  — store entries of matching `svc_type` not yet deployed locally
///   External   — always appended at the end
fn slot_entries_for(
    class_prefix:  &str,
    svc_type:      &str,
    services:      &[ServiceHandle],
    store_entries: &[StoreEntry],
) -> Vec<crate::ui::nodes::service_slot::SlotEntry> {
    use crate::ui::nodes::service_slot::SlotEntry;

    let mut entries = Vec::new();

    // 1. Already-deployed local instances of matching type
    for svc in services {
        if svc.config.service.service_class.starts_with(class_prefix) {
            entries.push(SlotEntry::configured(&svc.name, svc_type));
        }
    }

    // 2. Store modules of matching type not yet deployed locally.
    // Compare against service_class (the module ID), not the instance name,
    // so "kanidm-prod" correctly suppresses the "iam/kanidm" store entry.
    for entry in store_entries {
        if entry.primary_type_str() == svc_type {
            let already_deployed = services.iter()
                .any(|s| s.config.service.service_class == entry.id);
            if !already_deployed {
                entries.push(SlotEntry::available(&entry.id, &entry.name, svc_type));
            }
        }
    }

    // 3. Always append external option
    entries.push(SlotEntry::external());
    entries
}

/// Append ServiceSlotNode instances to a nodes list.
///
/// `slot_values` tuples: (key, label_key, class_prefix, svc_type, current_value)
fn append_slot_nodes(
    nodes:         &mut Vec<Box<dyn FormNode>>,
    services:      &[ServiceHandle],
    store_entries: &[StoreEntry],
    slot_values:   &[(&'static str, &'static str, &'static str, &'static str, &str)],
) {
    use crate::ui::nodes::service_slot::ServiceSlotNode;

    for &(key, label_key, class_prefix, svc_type, current_val) in slot_values {
        let entries = slot_entries_for(class_prefix, svc_type, services, store_entries);
        let mut node = ServiceSlotNode::new(key, label_key, 0, false, entries, svc_type)
            .col(6)
            .min_w(24);
        if !current_val.is_empty() {
            node = node.with_value(current_val);
        }
        nodes.push(Box::new(node));
    }
}

// ── Smart-defaults hook ───────────────────────────────────────────────────────

/// Derives domain from name and contact_email from domain automatically.
pub fn project_on_change(nodes: &mut Vec<Box<dyn FormNode>>, key: &'static str) {
    match key {
        "name" => {
            let name_val = nodes.iter().find(|n| n.key() == "name")
                .map(|n| n.value().to_string()).unwrap_or_default();
            let slug = crate::app::slugify(&name_val);

            let domain_dirty = nodes.iter().find(|n| n.key() == "domain")
                .map(|n| n.is_dirty()).unwrap_or(false);
            if !domain_dirty {
                if let Some(n) = nodes.iter_mut().find(|n| n.key() == "domain") {
                    n.set_value(&slug);
                }
            }
            sync_email_from_domain(nodes);
        }
        "domain" => sync_email_from_domain(nodes),
        _ => {}
    }
}

fn sync_email_from_domain(nodes: &mut Vec<Box<dyn FormNode>>) {
    let domain = nodes.iter().find(|n| n.key() == "domain")
        .map(|n| n.value().to_string()).unwrap_or_default();
    if domain.is_empty() { return; }
    let email_dirty = nodes.iter().find(|n| n.key() == "contact_email")
        .map(|n| n.is_dirty()).unwrap_or(false);
    if !email_dirty {
        if let Some(n) = nodes.iter_mut().find(|n| n.key() == "contact_email") {
            n.set_value(&format!("admin@{}", domain));
        }
    }
}

// ── Form builders ─────────────────────────────────────────────────────────────

/// Build the dynamic options list for the `languages` multi-select field.
///
/// Always includes "en" (English built-in), then all installed dynamic langs by code.
/// These are the languages the user has enabled in Settings → Languages.
pub fn build_lang_options(available_langs: &[&'static crate::i18n::DynamicLang]) -> Vec<String> {
    let mut opts = vec!["en".to_string()];
    for dl in available_langs {
        if dl.code != "en" {
            opts.push(dl.code.to_string());
        }
    }
    opts
}

pub fn new_project_form(
    services:       &[ServiceHandle],
    store_entries:  &[StoreEntry],
    available_langs: &[&'static crate::i18n::DynamicLang],
) -> ResourceForm {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".into());
    let dynamics: &[(&str, String)] = &[
        ("install_dir", format!("{}/fsn", home)),
    ];
    let lang_opts   = build_lang_options(available_langs);
    let class_opts  = [("languages", lang_opts)];
    let mut nodes = schema_form::build_nodes(
        ProjectFormData::schema(),
        &HashMap::new(),
        DISPLAY_FNS,
        dynamics,
        &class_opts,
    );
    append_slot_nodes(&mut nodes, services, store_entries, &[
        ("iam",        "form.project.iam",        "iam/",        "iam",        ""),
        ("wiki",       "form.project.wiki",       "wiki/",       "wiki",       ""),
        ("mail",       "form.project.mail",       "mail/",       "mail",       ""),
        ("monitoring", "form.project.monitoring", "monitoring/", "monitoring", ""),
        ("git",        "form.project.git",        "git/",        "git",        ""),
    ]);
    ResourceForm::new(ResourceKind::Project, PROJECT_TABS, nodes, None, project_on_change)
}

pub fn edit_project_form(
    handle:          &ProjectHandle,
    services:        &[ServiceHandle],
    store_entries:   &[StoreEntry],
    available_langs: &[&'static crate::i18n::DynamicLang],
) -> ResourceForm {
    let p    = &handle.config.project;
    let desc = p.meta.description.as_deref().unwrap_or("").to_string();
    let slots = &handle.config.services;
    let languages_str = p.languages.join(",");
    let tags_str      = p.meta.tags.join(",");
    let alias = p.meta.alias.as_deref().unwrap_or("");
    let prefill: HashMap<&str, &str> = [
        ("name",          p.meta.name.as_str()),
        ("domain",        p.domain.as_str()),
        ("alias",         alias),
        ("description",   desc.as_str()),
        ("contact_email", handle.email()),
        ("language",      p.language.as_str()),
        ("languages",     languages_str.as_str()),
        ("tags",          tags_str.as_str()),
        ("install_dir",   handle.install_dir()),
        ("version",       p.meta.version.as_str()),
    ].into_iter().filter(|(_, v)| !v.is_empty()).collect();

    let lang_opts  = build_lang_options(available_langs);
    let class_opts = [("languages", lang_opts)];
    let mut nodes = schema_form::build_nodes(
        ProjectFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &[],
        &class_opts,
    );
    append_slot_nodes(&mut nodes, services, store_entries, &[
        ("iam",        "form.project.iam",        "iam/",        "iam",        slots.iam.as_deref().unwrap_or("")),
        ("wiki",       "form.project.wiki",       "wiki/",       "wiki",       slots.wiki.as_deref().unwrap_or("")),
        ("mail",       "form.project.mail",       "mail/",       "mail",       slots.mail.as_deref().unwrap_or("")),
        ("monitoring", "form.project.monitoring", "monitoring/", "monitoring", slots.monitoring.as_deref().unwrap_or("")),
        ("git",        "form.project.git",        "git/",        "git",        slots.git.as_deref().unwrap_or("")),
    ]);
    ResourceForm::new(ResourceKind::Project, PROJECT_TABS, nodes, Some(handle.slug.clone()), project_on_change)
}

// ── Submit ────────────────────────────────────────────────────────────────────

// ── SlotValue — typed parse result for service-slot field values ──────────────
//
// Pattern: Single Source of Truth — all "new:" / "store:" / assigned / empty
// parsing lives here. Both submit.rs (task queuing) and submit_project_form()
// (TOML writing) use parse_slot_value() instead of duplicating the prefix checks.

/// Typed result of parsing a service-slot field value.
#[derive(Debug, Clone, PartialEq)]
pub enum SlotValue<'a> {
    /// Empty or unset — nothing to write or queue.
    Empty,
    /// "new:<class>" — user wants to create a new local service of this class.
    New { class: &'a str },
    /// "store:<class>" — user wants to install a module from the store.
    Store { class: &'a str },
    /// A concrete assignment (service name or "external") — write to TOML.
    Assigned(&'a str),
}

/// Parse a service-slot field value into its typed variant.
///
/// Single source of truth: edit here to change how slot strings are interpreted.
/// Called by submit.rs for task queuing and by submit_project_form() for TOML writing.
pub fn parse_slot_value(v: &str) -> SlotValue<'_> {
    if v.is_empty() {
        SlotValue::Empty
    } else if let Some(class) = v.strip_prefix("new:") {
        SlotValue::New { class }
    } else if let Some(class) = v.strip_prefix("store:") {
        SlotValue::Store { class }
    } else {
        SlotValue::Assigned(v)
    }
}

/// Returns Some(v) only for values that represent a real assignment
/// (service name or "external"). Filters out pending "new:" / "store:" values.
/// Delegates to parse_slot_value() — single source of truth.
fn clean_slot_value(v: &str) -> Option<&str> {
    match parse_slot_value(v) {
        SlotValue::Assigned(s) => Some(s),
        _ => None,
    }
}

pub fn submit_project_form(form: &ResourceForm, root: &Path) -> Result<()> {
    use toml::value::{Table, Value};

    let is_edit = form.edit_id.is_some();
    let slug = form.edit_id.clone()
        .unwrap_or_else(|| crate::app::slugify(&form.field_value("name")));

    let project_dir = root.join("projects").join(&slug);
    std::fs::create_dir_all(&project_dir)?;

    let toml_path = project_dir.join(format!("{}.project.toml", slug));
    if !is_edit && toml_path.exists() {
        anyhow::bail!("A project named '{}' already exists", slug);
    }

    let name      = form.field_value("name");
    let domain    = form.field_value("domain");
    let alias     = form.field_value("alias");
    let desc      = form.field_value("description");
    let email     = form.field_value("contact_email");
    let lang      = form.field_value("language");
    let languages = form.field_value("languages");
    let path      = form.field_value("install_dir");
    let version   = form.field_value("version");
    let tags      = form.field_value("tags");
    let svc_iam   = form.field_value("iam");
    let svc_wiki  = form.field_value("wiki");
    let svc_mail  = form.field_value("mail");
    let svc_mon   = form.field_value("monitoring");
    let svc_git   = form.field_value("git");

    // ── Build TOML document via toml::value::Table ────────────────────────
    // Using toml::ser instead of string formatting ensures that special
    // characters in field values (quotes, backslashes, unicode) are escaped
    // correctly and can never corrupt the TOML structure.

    let mut project_table = Table::new();
    project_table.insert("name".into(),        Value::String(name));
    project_table.insert("domain".into(),      Value::String(domain));
    if !alias.is_empty() {
        project_table.insert("alias".into(), Value::String(alias));
    }
    project_table.insert("description".into(), Value::String(desc));
    project_table.insert("language".into(),    Value::String(lang));
    project_table.insert("install_dir".into(), Value::String(path));
    project_table.insert("version".into(),     Value::String(version));

    if !languages.is_empty() {
        let lang_arr: Vec<Value> = languages.split(',')
            .map(|l| Value::String(l.trim().to_string()))
            .collect();
        project_table.insert("languages".into(), Value::Array(lang_arr));
    }

    if !tags.is_empty() {
        let tag_arr: Vec<Value> = tags.split(',')
            .map(|t| Value::String(t.trim().to_string()))
            .collect();
        project_table.insert("tags".into(), Value::Array(tag_arr));
    }

    // Contact email — [project.contact] sub-table
    if !email.is_empty() {
        let mut contact = Table::new();
        contact.insert("email".into(), Value::String(email));
        project_table.insert("contact".into(), Value::Table(contact));
    }

    let mut root_doc = Table::new();
    root_doc.insert("project".into(), Value::Table(project_table));

    // Service slots — only write non-empty, non-pending assignments
    let slot_pairs: &[(&str, String)] = &[
        ("iam",        svc_iam),
        ("wiki",       svc_wiki),
        ("mail",       svc_mail),
        ("monitoring", svc_mon),
        ("git",        svc_git),
    ];
    if slot_pairs.iter().any(|(_, v)| clean_slot_value(v).is_some()) {
        let mut services_table = Table::new();
        for (key, val) in slot_pairs {
            if let Some(v) = clean_slot_value(val) {
                services_table.insert((*key).into(), Value::String(v.to_string()));
            }
        }
        root_doc.insert("services".into(), Value::Table(services_table));
    }

    let file_content = toml::to_string_pretty(&Value::Table(root_doc))?;
    std::fs::write(&toml_path, file_content)?;
    Ok(())
}
