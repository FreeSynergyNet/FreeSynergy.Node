mod cli;
mod commands;
mod db;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

// Bundled locale strings compiled into the binary for offline-first i18n.
const LOCALE_EN: &str = include_str!("../locales/en/cli.toml");
const LOCALE_DE: &str = include_str!("../locales/de/cli.toml");

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing (controlled by RUST_LOG env var)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Global panic handler — log the panic via tracing instead of writing raw
    // to stderr so that structured log pipelines capture it, then abort.
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("(no message)");
        tracing::error!(panic.location = %location, panic.message = %message, "fsn panicked — this is a bug, please report it");
    }));

    // Detect system language from LANG / LANGUAGE env vars; default to "en".
    let lang = detect_lang();
    let _ = fsn_i18n::init_with_toml_strs(&lang, &[("en", LOCALE_EN), ("de", LOCALE_DE)]);

    // DB init (non-fatal: CLI works without persistence)
    if let Err(e) = db::init().await {
        tracing::warn!("DB unavailable: {e}");
    } else {
        db::spawn_flush_loop();
    }

    let result = cli::run().await;
    db::flush().await;
    result
}

/// Detect the active UI language from environment variables.
///
/// Reads `LANGUAGE`, `LANG`, or `LC_ALL` (in order of precedence) and
/// extracts the ISO 639-1 two-letter code.  Defaults to `"en"`.
fn detect_lang() -> String {
    let raw = std::env::var("LANGUAGE")
        .or_else(|_| std::env::var("LANG"))
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_default();

    // Take the first two characters of e.g. "de_DE.UTF-8" → "de"
    let code = raw.split(['.', '_']).next().unwrap_or("en");
    match code {
        "de" => "de".to_string(),
        _    => "en".to_string(),
    }
}
