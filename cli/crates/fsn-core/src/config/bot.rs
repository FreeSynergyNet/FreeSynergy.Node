// Bot configuration — automation agents running within a project.
//
// Examples: Matrix bot (Hookshot / Maubot), Telegram bot, webhook receiver.
// Referenced via ServiceInstanceMeta.bots: Vec<String>.
//
// No own .toml file yet — in-memory type only.
// A full file-backed config (with load()) can be added later when bots
// gain their own deployment lifecycle.

use serde::{Deserialize, Serialize};

use crate::config::meta::ResourceMeta;
use crate::error::FsnError;
use crate::resource::{BotResource, Resource, ResourcePhase};

// ── BotConfig ─────────────────────────────────────────────────────────────────

/// Root configuration for a bot / automation agent.
///
/// Bot instances are lightweight processes attached to a service instance.
/// They communicate via the service's API (Matrix, Telegram, webhooks, …).
///
/// Identified in the system by `BotMeta::name` within a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub bot: BotMeta,
}

// ── BotMeta ───────────────────────────────────────────────────────────────────

/// Core metadata for a bot instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotMeta {
    /// Common fields: name, alias, description, version, tags.
    #[serde(flatten)]
    pub meta: ResourceMeta,

    /// Integration type — determines which service API the bot uses.
    pub bot_type: BotType,

    /// Project slug this bot belongs to.
    pub project: String,

    /// Service class that runs this bot, e.g. `"bot/matrix-hookshot"`.
    pub service_class: String,
}

// ── BotType ───────────────────────────────────────────────────────────────────

/// Integration type for a bot.
///
/// Determines which external messaging system or protocol the bot uses.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BotType {
    /// Matrix bot (e.g. Hookshot, Maubot).
    Matrix,
    /// Telegram bot via the Bot API.
    Telegram,
    /// Generic HTTP webhook receiver / sender.
    Webhook,
    /// User-defined integration type.
    #[default]
    Custom,
}

impl BotType {
    /// Returns the machine-readable string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            BotType::Matrix   => "matrix",
            BotType::Telegram => "telegram",
            BotType::Webhook  => "webhook",
            BotType::Custom   => "custom",
        }
    }
}

// ── Resource impl ─────────────────────────────────────────────────────────────

impl Resource for BotConfig {
    fn kind(&self) -> &'static str { "bot" }
    fn id(&self) -> &str { &self.bot.meta.name }
    fn display_name(&self) -> &str { self.bot.meta.display_name() }
    fn description(&self) -> Option<&str> { self.bot.meta.description.as_deref() }
    fn tags(&self) -> &[String] { &self.bot.meta.tags }
    fn phase(&self) -> ResourcePhase { ResourcePhase::Unknown }

    fn validate(&self) -> Result<(), FsnError> {
        if self.bot.meta.name.is_empty() {
            return Err(FsnError::ConstraintViolation { message: "bot.name is required".into() });
        }
        if self.bot.project.is_empty() {
            return Err(FsnError::ConstraintViolation { message: "bot.project is required".into() });
        }
        if self.bot.service_class.is_empty() {
            return Err(FsnError::ConstraintViolation { message: "bot.service_class is required".into() });
        }
        Ok(())
    }
}

impl BotResource for BotConfig {
    fn project(&self)       -> &str { &self.bot.project }
    fn service_class(&self) -> &str { &self.bot.service_class }
    fn bot_type_str(&self)  -> &str { self.bot.bot_type.as_str() }
}
