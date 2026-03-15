// Database lifecycle management for the FSN CLI.
//
// Initializes a SQLite database at ~/.local/share/fsn/fsn.db, runs migrations,
// and provides a write buffer for async audit persistence.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use anyhow::{Context, Result};
use fsn_node_core::audit::AuditEntry;
use fsn_db::{BufferedWrite, DbBackend, DbConnection, Migrator, WriteBuffer};
use tracing::warn;

static DB: OnceLock<Arc<DbConnection>> = OnceLock::new();
static WRITE_BUF: OnceLock<Arc<WriteBuffer>> = OnceLock::new();

/// Path to the FSN SQLite database (`~/.local/share/fsn/fsn.db`).
pub fn db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".local/share/fsn/fsn.db")
}

/// Initialize the database: connect, run migrations, set up write buffer.
///
/// Call once at startup. Non-fatal — the CLI continues without persistence
/// if DB init fails (e.g. permission error, missing SQLite).
pub async fn init() -> Result<()> {
    let path = db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating DB directory {}", parent.display()))?;
    }

    let conn = DbConnection::connect(DbBackend::Sqlite {
        path: path.to_string_lossy().into_owned(),
    })
    .await
    .map_err(|e| anyhow::anyhow!("DB connect: {e}"))?;

    Migrator::run(conn.inner())
        .await
        .map_err(|e| anyhow::anyhow!("DB migrations: {e}"))?;

    let buf = WriteBuffer::with_defaults(conn.inner().clone());
    WRITE_BUF.set(Arc::new(buf)).ok();
    DB.set(Arc::new(conn)).ok();
    Ok(())
}

/// Spawn the write-buffer auto-flush loop as a background tokio task.
///
/// Call after `init()` succeeds. The task runs until the process exits.
pub fn spawn_flush_loop() {
    if let Some(buf) = WRITE_BUF.get() {
        let buf = buf.clone();
        tokio::spawn(async move { buf.run_auto_flush().await });
    }
}

/// Write an audit entry to the database via the write buffer.
///
/// Fire and forget — silently does nothing when the DB was not initialized.
pub async fn write_audit_entry(entry: &AuditEntry) {
    let Some(buf) = WRITE_BUF.get() else { return };

    // Escape single quotes for inline SQL (values are internal strings, not user input)
    let actor  = entry.actor.replace('\'', "''");
    let action = entry.action.replace('\'', "''");
    let kind   = entry.resource_kind.replace('\'', "''");
    let payload = match &entry.detail {
        Some(d) => format!("'{}'", d.replace('\'', "''")),
        None    => "NULL".to_string(),
    };

    let sql = format!(
        "INSERT INTO audit_logs (actor, action, resource_kind, payload, outcome, created_at) \
         VALUES ('{actor}', '{action}', '{kind}', {payload}, 'ok', {})",
        entry.timestamp,
    );

    if let Err(e) = buf.enqueue(BufferedWrite { sql, values: vec![] }).await {
        warn!("audit write failed: {e}");
    }
}

/// Flush all pending writes to disk.
///
/// Call before process exit to ensure the last audit entries are persisted.
pub async fn flush() {
    if let Some(buf) = WRITE_BUF.get() {
        if let Err(e) = buf.flush().await {
            warn!("final DB flush failed: {e}");
        }
    }
}
