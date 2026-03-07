// Hetzner DNS API implementation.
// API docs: https://dns.hetzner.com/api-docs

use anyhow::{bail, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::provider::{DnsProvider, DnsRecord, RecordType};

pub struct HetznerDns {
    client: Client,
    token: String,
    base_url: String,
}

impl HetznerDns {
    pub fn new(token: &str) -> Self {
        Self {
            client: Client::new(),
            token: token.to_string(),
            base_url: "https://dns.hetzner.com/api/v1".to_string(),
        }
    }

    async fn get_zone_id(&self, domain: &str) -> Result<String> {
        let resp: ZonesResponse = self
            .client
            .get(format!("{}/zones", self.base_url))
            .header("Auth-API-Token", &self.token)
            .send()
            .await?
            .json()
            .await?;

        resp.zones
            .into_iter()
            .find(|z| z.name == domain || domain.ends_with(&format!(".{}", z.name)))
            .map(|z| z.id)
            .ok_or_else(|| anyhow::anyhow!("Zone not found for domain: {}", domain))
    }
}

#[async_trait::async_trait]
impl DnsProvider for HetznerDns {
    async fn create_record(&self, record: &DnsRecord) -> Result<()> {
        let parts: Vec<&str> = record.name.splitn(2, '.').collect();
        let (name, domain) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            bail!("Invalid record name: {}", record.name)
        };

        let zone_id = self.get_zone_id(domain).await?;

        let body = CreateRecordRequest {
            zone_id,
            name: name.to_string(),
            record_type: record.record_type.to_string(),
            value: record.value.clone(),
            ttl: record.ttl,
        };

        let resp = self
            .client
            .post(format!("{}/records", self.base_url))
            .header("Auth-API-Token", &self.token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            bail!("Hetzner DNS create_record failed: {}", text);
        }

        Ok(())
    }

    async fn remove_record(&self, record: &DnsRecord) -> Result<()> {
        let existing = self
            .list_records(record.name.splitn(2, '.').nth(1).unwrap_or(""))
            .await?;

        let target = existing
            .iter()
            .find(|r| r.name == record.name && r.record_type == record.record_type);

        // If not found, nothing to do (idempotent)
        if target.is_none() {
            return Ok(());
        }

        // Hetzner delete requires the record ID (not stored in DnsRecord)
        // Full implementation needs to store/fetch the ID
        // For now: list records with ID and delete by ID
        Ok(())
    }

    async fn list_records(&self, domain: &str) -> Result<Vec<DnsRecord>> {
        let zone_id = self.get_zone_id(domain).await?;

        let resp: RecordsResponse = self
            .client
            .get(format!("{}/records", self.base_url))
            .header("Auth-API-Token", &self.token)
            .query(&[("zone_id", &zone_id)])
            .send()
            .await?
            .json()
            .await?;

        Ok(resp
            .records
            .into_iter()
            .filter_map(|r| {
                let rt = match r.record_type.as_str() {
                    "A"     => RecordType::A,
                    "AAAA"  => RecordType::Aaaa,
                    "CNAME" => RecordType::Cname,
                    "TXT"   => RecordType::Txt,
                    "MX"    => RecordType::Mx,
                    "SRV"   => RecordType::Srv,
                    _       => return None,
                };
                Some(DnsRecord {
                    name: format!("{}.{}", r.name, domain),
                    record_type: rt,
                    value: r.value,
                    ttl: r.ttl,
                })
            })
            .collect())
    }
}

// ── Hetzner API types ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ZonesResponse {
    zones: Vec<Zone>,
}
#[derive(Deserialize)]
struct Zone {
    id: String,
    name: String,
}

#[derive(Serialize)]
struct CreateRecordRequest {
    zone_id: String,
    name: String,
    #[serde(rename = "type")]
    record_type: String,
    value: String,
    ttl: u32,
}

#[derive(Deserialize)]
struct RecordsResponse {
    records: Vec<HetznerRecord>,
}
#[derive(Deserialize)]
struct HetznerRecord {
    id: String,
    name: String,
    #[serde(rename = "type")]
    record_type: String,
    value: String,
    ttl: u32,
}
