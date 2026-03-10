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

    // Version (col=3) + UI Language (col=4) + Install Path (col=5) in one row.
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
    #[form(label = "form.project.languages", widget = "multi_select", tab = 0,
           hint = "form.project.languages.hint",
           options = "de,en,fr,es,it,pt,ru,zh,ja,ar")]
    pub languages: String,

    // ── Section: Services ─────────────────────────────────────────────────
    #[form(widget = "section", label = "form.section.services", tab = 0)]
    pub _section_services: String,

    // Service slots — each is a Select from all available services of that type.
    // Options are populated dynamically at form-build time from loaded service instances.
    // Multiple services of the same type CAN be installed; this slot designates
    // which one serves as the "primary" for that role (proxy routing, auto-docs, etc.).

    #[form(label = "form.project.iam", widget = "select", tab = 0, col = 6, min_w = 24,
           hint = "form.project.iam.hint")]
    pub iam: String,

    #[form(label = "form.project.wiki", widget = "select", tab = 0, col = 6, min_w = 24,
           hint = "form.project.wiki.hint")]
    pub wiki: String,

    #[form(label = "form.project.mail", widget = "select", tab = 0, col = 6, min_w = 24,
           hint = "form.project.mail.hint")]
    pub mail: String,

    #[form(label = "form.project.monitoring", widget = "select", tab = 0, col = 6, min_w = 24,
           hint = "form.project.monitoring.hint")]
    pub monitoring: String,

    #[form(label = "form.project.git", widget = "select", tab = 0, col = 6, min_w = 24,
           hint = "form.project.git.hint")]
    pub git: String,
}

// ── Display helpers ───────────────────────────────────────────────────────────

pub fn lang_display(code: &str) -> &'static str {
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
        _    => "—",
    }
}

/// Display label for service-slot select fields.
/// Returns "" for unknown instance names (SelectInputNode falls back to raw value).
pub fn slot_display(code: &str) -> &'static str {
    match code {
        ""         => "—",
        "external" => "Externer Service",
        _          => "",  // raw instance name shown as-is
    }
}

const DISPLAY_FNS: &[(&str, fn(&str) -> &'static str)] = &[
    ("language",   lang_display),
    ("languages",  lang_display),
    ("iam",        slot_display),
    ("wiki",       slot_display),
    ("mail",       slot_display),
    ("monitoring", slot_display),
    ("git",        slot_display),
];

/// Build the dropdown options for a service slot.
///
/// Includes (in order):
///   ""             — not configured (shown as "—")
///   {instance}     — each locally deployed service whose class starts with `class_prefix`
///   {module_name}  — each Store entry of matching service_type, not yet deployed locally
///   "external"     — externally hosted service
fn slot_options(
    class_prefix:  &str,
    service_type:  &str,
    services:      &[ServiceHandle],
    store_entries: &[StoreEntry],
) -> Vec<String> {
    let mut opts = vec!["".to_string()];

    // 1. Already-deployed local instances of matching type
    for svc in services {
        if svc.config.service.service_class.starts_with(class_prefix) {
            opts.push(svc.name.clone());
        }
    }

    // 2. Store modules of matching type that are not already listed
    for entry in store_entries {
        if entry.service_type == service_type {
            // Extract the module short-name ("iam/kanidm" → "kanidm")
            let module_name = entry.id.split('/').last().unwrap_or(&entry.id).to_string();
            if !opts.contains(&module_name) {
                opts.push(module_name);
            }
        }
    }

    opts.push("external".to_string());
    opts
}

/// Build the full dynamic_options slice for all service slot fields.
fn build_slot_options(
    services:      &[ServiceHandle],
    store_entries: &[StoreEntry],
) -> Vec<(&'static str, Vec<String>)> {
    vec![
        ("iam",        slot_options("iam/",        "iam",        services, store_entries)),
        ("wiki",       slot_options("wiki/",       "wiki",       services, store_entries)),
        ("mail",       slot_options("mail/",       "mail",       services, store_entries)),
        ("monitoring", slot_options("monitoring/", "monitoring", services, store_entries)),
        ("git",        slot_options("git/",        "git",        services, store_entries)),
    ]
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

pub fn new_project_form(services: &[ServiceHandle], store_entries: &[StoreEntry]) -> ResourceForm {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".into());
    let dynamics: &[(&str, String)] = &[
        ("install_dir", format!("{}/fsn", home)),
    ];
    let dyn_opts = build_slot_options(services, store_entries);
    let nodes = schema_form::build_nodes(
        ProjectFormData::schema(),
        &HashMap::new(),
        DISPLAY_FNS,
        dynamics,
        &dyn_opts,
    );
    ResourceForm::new(ResourceKind::Project, PROJECT_TABS, nodes, None, project_on_change)
}

pub fn edit_project_form(
    handle:        &ProjectHandle,
    services:      &[ServiceHandle],
    store_entries: &[StoreEntry],
) -> ResourceForm {
    let p    = &handle.config.project;
    let desc = p.description.as_deref().unwrap_or("").to_string();
    let slots = &handle.config.services;
    let languages_str = p.languages.join(",");
    let prefill: HashMap<&str, &str> = [
        ("name",          p.name.as_str()),
        ("domain",        p.domain.as_str()),
        ("description",   desc.as_str()),
        ("contact_email", handle.email()),
        ("language",      p.language.as_str()),
        ("languages",     languages_str.as_str()),
        ("install_dir",   handle.install_dir()),
        ("version",       p.version.as_str()),
        ("iam",           slots.iam.as_deref().unwrap_or("")),
        ("wiki",          slots.wiki.as_deref().unwrap_or("")),
        ("mail",          slots.mail.as_deref().unwrap_or("")),
        ("monitoring",    slots.monitoring.as_deref().unwrap_or("")),
        ("git",           slots.git.as_deref().unwrap_or("")),
    ].into_iter().filter(|(_, v)| !v.is_empty()).collect();

    let dyn_opts = build_slot_options(services, store_entries);
    let nodes = schema_form::build_nodes(
        ProjectFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &[],
        &dyn_opts,
    );
    ResourceForm::new(ResourceKind::Project, PROJECT_TABS, nodes, Some(handle.slug.clone()), project_on_change)
}

// ── Submit ────────────────────────────────────────────────────────────────────

pub fn submit_project_form(form: &ResourceForm, root: &Path) -> Result<()> {
    let is_edit = form.edit_id.is_some();
    let slug = form.edit_id.clone()
        .unwrap_or_else(|| crate::app::slugify(&form.field_value("name")));

    let project_dir = root.join("projects").join(&slug);
    std::fs::create_dir_all(&project_dir)?;

    let toml_path = project_dir.join(format!("{}.project.toml", slug));
    if !is_edit && toml_path.exists() { return Ok(()); }

    let name       = form.field_value("name");
    let domain     = form.field_value("domain");
    let desc       = form.field_value("description");
    let email      = form.field_value("contact_email");
    let lang       = form.field_value("language");
    let languages  = form.field_value("languages");
    let path       = form.field_value("install_dir");
    let version    = form.field_value("version");
    let tags       = form.field_value("tags");
    let svc_iam    = form.field_value("iam");
    let svc_wiki   = form.field_value("wiki");
    let svc_mail   = form.field_value("mail");
    let svc_mon    = form.field_value("monitoring");
    let svc_git    = form.field_value("git");

    let mut file_content = format!(
        "[project]\nname        = \"{name}\"\ndomain      = \"{domain}\"\ndescription = \"{desc}\"\nlanguage    = \"{lang}\"\ninstall_dir = \"{path}\"\nversion     = \"{version}\"\n"
    );

    // Languages — Vec<String> of content languages supported by this project
    if !languages.is_empty() {
        let lang_list: String = languages.split(',')
            .map(|l| format!("\"{}\"", l.trim()))
            .collect::<Vec<_>>().join(", ");
        file_content.push_str(&format!("languages   = [{lang_list}]\n"));
    }

    // Tags — Vec<String> field on ProjectMeta
    if !tags.is_empty() {
        let tag_list: String = tags.split(',')
            .map(|t| format!("\"{}\"", t.trim()))
            .collect::<Vec<_>>().join(", ");
        file_content.push_str(&format!("tags        = [{tag_list}]\n"));
    }

    // Contact email — written as [project.contact] sub-table (not a direct field on ProjectMeta)
    if !email.is_empty() {
        file_content.push_str(&format!("\n[project.contact]\nemail = \"{email}\"\n"));
    }

    // Service slots — only write non-empty assignments
    let has_slots = [svc_iam.as_str(), svc_wiki.as_str(), svc_mail.as_str(), svc_mon.as_str(), svc_git.as_str()]
        .iter().any(|v| !v.is_empty());
    if has_slots {
        file_content.push_str("\n[services]\n");
        if !svc_iam.is_empty()  { file_content.push_str(&format!("iam        = \"{svc_iam}\"\n")); }
        if !svc_wiki.is_empty() { file_content.push_str(&format!("wiki       = \"{svc_wiki}\"\n")); }
        if !svc_mail.is_empty() { file_content.push_str(&format!("mail       = \"{svc_mail}\"\n")); }
        if !svc_mon.is_empty()  { file_content.push_str(&format!("monitoring = \"{svc_mon}\"\n")); }
        if !svc_git.is_empty()  { file_content.push_str(&format!("git        = \"{svc_git}\"\n")); }
    }

    std::fs::write(&toml_path, file_content)?;
    Ok(())
}
