// Observe actual state – query systemd and podman for what is running.
// Replaces sync-stack.yml

use anyhow::Result;
use fsn_container::{SystemdManager, UnitActiveState};
use fsn_node_core::state::{ActualState, HealthStatus, RunState, ServiceStatus};

/// Query the current state of all FSN-managed services on this host.
pub async fn observe() -> Result<ActualState> {
    let systemd = SystemdManager::new();
    let unit_names = list_fsn_units(&systemd).await?;

    let mut services = Vec::with_capacity(unit_names.len());
    for unit in &unit_names {
        // Strip ".service" suffix to get the instance name.
        let name = unit.trim_end_matches(".service").to_string();

        let run_state = match systemd.status(unit).await {
            Ok(s) => match s.active_state {
                UnitActiveState::Active                => RunState::Running,
                UnitActiveState::Inactive
                | UnitActiveState::Deactivating        => RunState::Stopped,
                UnitActiveState::Failed                => RunState::Failed,
                UnitActiveState::Activating
                | UnitActiveState::Unknown             => RunState::Missing,
            },
            Err(_) => RunState::Missing,
        };

        services.push(ServiceStatus {
            name,
            state: run_state,
            health: HealthStatus::Unknown,   // HTTP health check is a separate step
            deployed_version: read_deployed_version(unit).unwrap_or_default(),
            container_id: None,
        });
    }

    Ok(ActualState { services })
}

/// List all active FSN-managed user units (units loaded by systemd --user).
///
/// Returns only `.service` units — same behaviour as the old `fsn_podman::systemd::list_fsn_units`.
pub async fn list_fsn_units(systemd: &SystemdManager) -> Result<Vec<String>> {
    let output = tokio::process::Command::new("systemctl")
        .args(["--user", "--type=service", "--state=loaded", "--plain", "--no-legend", "--no-pager"])
        .output()
        .await?;

    let _ = systemd; // used for type constraint; actual call is via subprocess
    let units = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let unit = line.split_whitespace().next()?;
            if unit.ends_with(".service") { Some(unit.to_string()) } else { None }
        })
        .collect();

    Ok(units)
}

/// Read the deployed version from the state marker file.
fn read_deployed_version(unit_name: &str) -> Option<String> {
    let name = unit_name.trim_end_matches(".service");
    let home = std::env::var("HOME").ok()?;
    let path = std::path::PathBuf::from(home)
        .join(".local/share/fsn/deployed")
        .join(format!("{}.version", name));
    std::fs::read_to_string(path).ok()?.lines().next().map(str::to_owned)
}
