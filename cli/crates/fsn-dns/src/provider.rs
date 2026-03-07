// DNS provider trait and record types.

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub name: String,        // e.g. "forgejo.example.com"
    pub record_type: RecordType,
    pub value: String,       // IP address or CNAME target
    pub ttl: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecordType {
    A,
    Aaaa,
    Cname,
    Txt,
    /// Mail exchanger record.  `DnsRecord::value` = priority + space + hostname,
    /// e.g. "10 mail.example.com."
    Mx,
    /// Service locator.  `DnsRecord::value` = "priority weight port target",
    /// e.g. "10 1 587 mail.example.com."
    Srv,
}

impl std::fmt::Display for RecordType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordType::A     => write!(f, "A"),
            RecordType::Aaaa  => write!(f, "AAAA"),
            RecordType::Cname => write!(f, "CNAME"),
            RecordType::Txt   => write!(f, "TXT"),
            RecordType::Mx    => write!(f, "MX"),
            RecordType::Srv   => write!(f, "SRV"),
        }
    }
}

/// Common interface for all DNS providers.
#[async_trait::async_trait]
pub trait DnsProvider: Send + Sync {
    async fn create_record(&self, record: &DnsRecord) -> Result<()>;
    async fn remove_record(&self, record: &DnsRecord) -> Result<()>;
    async fn list_records(&self, domain: &str) -> Result<Vec<DnsRecord>>;

    /// Reconcile: ensure desired records exist, remove stale ones.
    async fn reconcile(&self, desired: &[DnsRecord], domain: &str) -> Result<()> {
        let existing = self.list_records(domain).await?;

        for record in desired {
            let exists = existing
                .iter()
                .any(|r| r.name == record.name && r.record_type == record.record_type);
            if !exists {
                self.create_record(record).await?;
            }
        }

        // Remove records that are in existing but not in desired
        for record in &existing {
            let still_desired = desired
                .iter()
                .any(|r| r.name == record.name && r.record_type == record.record_type);
            if !still_desired {
                self.remove_record(record).await?;
            }
        }

        Ok(())
    }
}
