// Bot form — create / edit bot automation agents.
//
// Tabs:
//   Tab 0 (Bot): name, type (select), service_class (select), description
//   Tab 1 (Options): tags

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use fsn_form::Form;

use crate::app::{BOT_TABS, ResourceForm, ResourceKind};
use crate::schema_form;
use crate::ui::form_node::FormNode;

// ── Form schema ───────────────────────────────────────────────────────────────

/// Form schema for creating and editing a Bot.
#[derive(Form)]
pub struct BotFormData {
    // ── Tab 0: Bot ───────────────────────────────────────────────────────
    #[form(label = "form.bot.name", required, tab = 0, hint = "form.bot.name.hint")]
    pub name: String,

    #[form(label = "form.bot.type", widget = "select", required, tab = 0,
           options = "matrix,telegram,webhook,custom",
           default = "matrix")]
    pub bot_type: String,

    #[form(label = "form.bot.class", widget = "select", required, tab = 0,
           options = "bot/matrix-hookshot,bot/maubot,bot/telegram,bot/webhook",
           default = "bot/matrix-hookshot")]
    pub service_class: String,

    #[form(label = "form.bot.description", widget = "textarea", rows = 3, tab = 0)]
    pub description: String,

    // ── Tab 1: Options ───────────────────────────────────────────────────
    #[form(label = "form.bot.tags", tab = 1, hint = "form.bot.tags.hint")]
    pub tags: String,
}

// ── Display helpers ───────────────────────────────────────────────────────────

pub fn bot_type_display(code: &str) -> &'static str {
    match code {
        "matrix"   => "Matrix",
        "telegram" => "Telegram",
        "webhook"  => "Webhook",
        "custom"   => "Custom",
        _          => "—",
    }
}

pub fn bot_class_display(code: &str) -> &'static str {
    match code {
        "bot/matrix-hookshot" => "Matrix Hookshot",
        "bot/maubot"          => "Maubot",
        "bot/telegram"        => "Telegram Bot",
        "bot/webhook"         => "Webhook",
        _                     => "—",
    }
}

const DISPLAY_FNS: &[(&str, fn(&str) -> &'static str)] = &[
    ("bot_type",     bot_type_display),
    ("service_class", bot_class_display),
];

// ── Smart-defaults hook ───────────────────────────────────────────────────────

fn bot_on_change(nodes: &mut Vec<Box<dyn FormNode>>, key: &'static str) {
    // When bot_type changes, update service_class default suggestion
    if key == "bot_type" {
        let type_val = nodes.iter().find(|n| n.key() == "bot_type")
            .map(|n| n.value().to_string()).unwrap_or_default();
        let class_dirty = nodes.iter().find(|n| n.key() == "service_class")
            .map(|n| n.is_dirty()).unwrap_or(false);
        if !class_dirty {
            let suggested = match type_val.as_str() {
                "matrix"   => "bot/matrix-hookshot",
                "telegram" => "bot/telegram",
                "webhook"  => "bot/webhook",
                _          => "bot/matrix-hookshot",
            };
            if let Some(n) = nodes.iter_mut().find(|n| n.key() == "service_class") {
                n.set_value(suggested);
            }
        }
    }
}

// ── Form builder ──────────────────────────────────────────────────────────────

pub fn new_bot_form() -> ResourceForm {
    let nodes = schema_form::build_nodes(
        BotFormData::schema(),
        &HashMap::new(),
        DISPLAY_FNS,
        &[],
        &[],
    );
    ResourceForm::new(ResourceKind::Bot, BOT_TABS, nodes, None, bot_on_change)
}

// ── Submit ────────────────────────────────────────────────────────────────────

pub fn submit_bot_form(form: &ResourceForm, project_dir: &Path, project_slug: &str) -> Result<()> {
    let name          = form.field_value("name");
    let bot_type      = form.field_value("bot_type");
    let service_class = form.field_value("service_class");
    let description   = form.field_value("description");
    let tags          = form.field_value("tags");

    if name.is_empty()          { anyhow::bail!("Bot-Name ist erforderlich"); }
    if service_class.is_empty() { anyhow::bail!("Bot-Klasse ist erforderlich"); }

    let bots_dir = project_dir.join("bots");
    std::fs::create_dir_all(&bots_dir)?;

    let slug = crate::app::slugify(&name);
    let path = bots_dir.join(format!("{}.bot.toml", slug));

    let mut content = format!(
        "[bot]\nname          = \"{name}\"\nbot_type      = \"{bot_type}\"\nservice_class = \"{service_class}\"\nproject       = \"{project_slug}\"\n"
    );
    if !description.is_empty() {
        content.push_str(&format!("description   = \"{description}\"\n"));
    }
    if !tags.is_empty() {
        let tag_list: String = tags.split(',')
            .map(|t| format!("\"{}\"", t.trim()))
            .collect::<Vec<_>>()
            .join(", ");
        content.push_str(&format!("tags          = [{tag_list}]\n"));
    }

    std::fs::write(&path, content)?;
    Ok(())
}
