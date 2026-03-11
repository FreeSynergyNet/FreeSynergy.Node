// Settings store form — edit a single StoreConfig entry.
//
// Design Pattern: Same as host_form.rs / service_form.rs.
// #[derive(Form)] defines the schema; schema_form::build_nodes() creates the nodes.
// submit_store_form() reads values back and updates AppSettings in place.
//
// edit_id stores the index (usize) of the store in settings.stores as a string.

use std::collections::HashMap;

use anyhow::Result;
use fsn_form::Form;

use crate::app::{ResourceForm, ResourceKind, STORE_TABS};
use crate::schema_form;

// ── Form schema ───────────────────────────────────────────────────────────────

#[derive(Form)]
pub struct StoreFormData {
    #[form(label = "settings.store.name", required, tab = 0, hint = "settings.store.name.hint")]
    pub name: String,

    #[form(label = "settings.store.url", required, tab = 0, hint = "settings.store.url.hint")]
    pub url: String,

    #[form(label = "settings.store.git_url", tab = 0, hint = "settings.store.git_url.hint")]
    pub git_url: String,

    #[form(label = "settings.store.local_path", tab = 0, widget = "dir_picker",
           hint = "settings.store.local_path.hint")]
    pub local_path: String,

    #[form(label = "settings.store.enabled", widget = "select", tab = 0,
           options = "true,false", default = "true")]
    pub enabled: String,
}

// ── Display helpers ───────────────────────────────────────────────────────────

fn enabled_display(code: &str) -> &'static str {
    match code {
        "true"  => "Enabled",
        "false" => "Disabled",
        _       => "—",
    }
}

const DISPLAY_FNS: &[(&str, fn(&str) -> &'static str)] = &[
    ("enabled", enabled_display),
];

// ── Form builder ──────────────────────────────────────────────────────────────

/// Build an edit form for the store at `idx` in settings.stores.
pub fn edit_store_form(idx: usize, store: &fsn_core::config::StoreConfig) -> ResourceForm {
    let enabled_str = if store.enabled { "true" } else { "false" };
    let prefill: HashMap<&str, &str> = [
        ("name",       store.name.as_str()),
        ("url",        store.url.as_str()),
        ("git_url",    store.git_url.as_deref().unwrap_or("")),
        ("local_path", store.local_path.as_deref().unwrap_or("")),
        ("enabled",    enabled_str),
    ].into_iter().filter(|(_, v)| !v.is_empty()).collect();

    let nodes = schema_form::build_nodes(
        StoreFormData::schema(),
        &prefill,
        DISPLAY_FNS,
        &[],
        &[],
    );
    ResourceForm::new(ResourceKind::Store, STORE_TABS, nodes, Some(idx.to_string()), |_, _| {})
}

// ── Submit ────────────────────────────────────────────────────────────────────

/// Write form values back into the settings and persist to disk.
pub fn submit_store_form(form: &ResourceForm, settings: &mut fsn_core::config::AppSettings) -> Result<()> {
    let idx: usize = form.edit_id.as_ref()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("invalid store index"))?;

    let store = settings.stores.get_mut(idx)
        .ok_or_else(|| anyhow::anyhow!("store index out of range"))?;

    store.name       = form.field_value("name");
    store.url        = form.field_value("url");
    store.git_url    = { let v = form.field_value("git_url");    if v.is_empty() { None } else { Some(v) } };
    store.local_path = { let v = form.field_value("local_path"); if v.is_empty() { None } else { Some(v) } };
    store.enabled    = form.field_value("enabled") == "true";

    settings.save().map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}
