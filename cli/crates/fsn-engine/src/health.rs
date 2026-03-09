// HTTP health checking for deployed services.
// Replaces Ansible's "wait_for" + health polling pattern.

use std::time::Duration;

use anyhow::{bail, Result};
use fsn_core::state::{desired::ServiceInstance, HealthStatus};
use tracing::debug;

const _DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL:    Duration = Duration::from_secs(3);

/// Wait until the service is reachable and returns a successful HTTP response,
/// or until `timeout` is exceeded.
pub async fn wait_for_ready(instance: &ServiceInstance, timeout: Duration) -> Result<()> {
    let Some(path) = &instance.class.meta.health_path else {
        debug!("{}: no health_path configured, skipping health check", instance.name);
        return Ok(());
    };

    let port   = instance.class.meta.health_port.unwrap_or(instance.class.meta.port);
    let scheme = instance.class.meta.health_scheme.as_deref().unwrap_or("http");
    let url    = format!("{}://localhost:{}{}", scheme, port, path);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true) // container may use self-signed
        .build()?;

    let deadline = std::time::Instant::now() + timeout;

    loop {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 401 => {
                // 401 counts as "up" – the service is responding, just needs auth
                tracing::info!("{}: health check passed ({})", instance.name, resp.status());
                return Ok(());
            }
            Ok(resp) => {
                debug!("{}: health check returned {} – retrying…", instance.name, resp.status());
            }
            Err(e) => {
                debug!("{}: health check error: {} – retrying…", instance.name, e);
            }
        }

        if std::time::Instant::now() >= deadline {
            bail!(
                "Service '{}' did not become healthy within {}s (url: {})",
                instance.name,
                timeout.as_secs(),
                url
            );
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Single health check without waiting – returns current status.
pub async fn check_once(instance: &ServiceInstance) -> HealthStatus {
    let Some(path) = &instance.class.meta.health_path else {
        return HealthStatus::Unknown;
    };
    let port   = instance.class.meta.health_port.unwrap_or(instance.class.meta.port);
    let scheme = instance.class.meta.health_scheme.as_deref().unwrap_or("http");
    let url    = format!("{}://localhost:{}{}", scheme, port, path);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    match client.get(&url).send().await {
        Ok(r) if r.status().is_success() || r.status().as_u16() == 401 => HealthStatus::Healthy,
        Ok(_) => HealthStatus::Unhealthy,
        Err(_) => HealthStatus::Unhealthy,
    }
}
