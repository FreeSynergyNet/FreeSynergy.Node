// audit.rs — Append-only audit log for deployment actions.
//
// AuditEntry records who did what, to which resource, and when.
//
// Current scope: in-process, in-memory log.
// CRDT-based multi-node sync (via fsn-sync + fsn-db) is Phase 2.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

// ── AuditEntry ────────────────────────────────────────────────────────────────

/// A single immutable audit record.
///
/// Fields are intentionally flat strings so the entry can be serialised
/// to any sink (TOML, JSON, database row) without schema migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unix timestamp (seconds since epoch) when the action occurred.
    pub timestamp: u64,

    /// Actor that triggered the action: username, bot name, or `"system"`.
    pub actor: String,

    /// The action verb: `"deploy"`, `"undeploy"`, `"update"`, `"init"`, …
    pub action: String,

    /// Resource kind: `"project"`, `"service"`, `"host"`, `"plugin"`.
    pub resource_kind: String,

    /// Resource identifier (project name, service name, host name, …).
    pub resource_name: String,

    /// Optional extra context: error message, version string, diff summary, …
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl AuditEntry {
    /// Create a new entry stamped with the current system time.
    pub fn new(
        actor:         impl Into<String>,
        action:        impl Into<String>,
        resource_kind: impl Into<String>,
        resource_name: impl Into<String>,
    ) -> Self {
        Self {
            timestamp:     now_unix_secs(),
            actor:         actor.into(),
            action:        action.into(),
            resource_kind: resource_kind.into(),
            resource_name: resource_name.into(),
            detail:        None,
        }
    }

    /// Attach an optional detail string and return `self` (builder style).
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

// ── AuditLog ──────────────────────────────────────────────────────────────────

/// In-memory, append-only audit log.
///
/// Persisting entries to disk and replicating them across nodes via CRDT
/// (Automerge through `fsn-sync`) is deferred to Phase 2.
#[derive(Debug, Default)]
pub struct AuditLog {
    entries: Vec<AuditEntry>,
}

impl AuditLog {
    /// Create an empty audit log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an entry.
    pub fn record(&mut self, entry: AuditEntry) {
        self.entries.push(entry);
    }

    /// Convenience: build and record an entry in one call.
    pub fn log(
        &mut self,
        actor:         impl Into<String>,
        action:        impl Into<String>,
        resource_kind: impl Into<String>,
        resource_name: impl Into<String>,
    ) {
        self.record(AuditEntry::new(actor, action, resource_kind, resource_name));
    }

    /// All entries in insertion order.
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Entries where `actor` matches.
    pub fn by_actor<'a>(&'a self, actor: &str) -> Vec<&'a AuditEntry> {
        self.entries.iter().filter(|e| e.actor == actor).collect()
    }

    /// Entries where `action` matches.
    pub fn by_action<'a>(&'a self, action: &str) -> Vec<&'a AuditEntry> {
        self.entries.iter().filter(|e| e.action == action).collect()
    }

    /// Entries for a specific resource (kind + name pair).
    pub fn by_resource<'a>(&'a self, kind: &str, name: &str) -> Vec<&'a AuditEntry> {
        self.entries.iter()
            .filter(|e| e.resource_kind == kind && e.resource_name == name)
            .collect()
    }
}

// ── private helpers ───────────────────────────────────────────────────────────

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_filter() {
        let mut log = AuditLog::new();
        log.record(AuditEntry::new("alice", "deploy",   "project", "my-project"));
        log.record(AuditEntry::new("system", "update",  "service", "kanidm").with_detail("v1.2.3"));
        log.record(AuditEntry::new("alice",  "undeploy", "service", "kanidm"));

        assert_eq!(log.entries().len(), 3);
        assert_eq!(log.by_actor("alice").len(), 2);
        assert_eq!(log.by_action("deploy").len(), 1);
        assert_eq!(log.by_resource("service", "kanidm").len(), 2);
        assert_eq!(log.entries()[1].detail.as_deref(), Some("v1.2.3"));
    }

    #[test]
    fn log_convenience_method() {
        let mut log = AuditLog::new();
        log.log("system", "init", "project", "test");
        assert_eq!(log.entries()[0].action, "init");
        assert!(log.entries()[0].timestamp > 0);
    }

    #[test]
    fn serialise_round_trip() {
        let entry = AuditEntry::new("bot", "deploy", "service", "forgejo")
            .with_detail("v7.0.1");
        let json = serde_json::to_string(&entry).unwrap();
        let decoded: AuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.actor, "bot");
        assert_eq!(decoded.detail.as_deref(), Some("v7.0.1"));
    }
}
