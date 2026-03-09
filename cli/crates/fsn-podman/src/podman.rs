// Podman container management via subprocess.

use anyhow::Result;
use fsn_core::state::actual::{HealthStatus, RunState};

#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub name: String,
    pub id: String,
    pub state: RunState,
    pub health: HealthStatus,
    pub image: String,
}

/// List all containers whose names match the FSN naming convention.
/// In Phase 1, returns empty (Ansible owns this).
pub async fn list_fsn_containers(_project_name: &str) -> Result<Vec<ContainerInfo>> {
    // Phase 2: run `podman ps --format json --filter label=fsn.project={project_name}`
    // and parse output
    Ok(Vec::new())
}

/// Get info for a single container by name.
pub async fn container_info(_name: &str) -> Result<Option<ContainerInfo>> {
    // Phase 2: `podman inspect {name} --format json`
    Ok(None)
}

/// Pull an image (used by update operation).
pub async fn pull_image(image: &str, tag: &str) -> Result<()> {
    use tokio::process::Command;
    let status = Command::new("podman")
        .args(["pull", &format!("{}:{}", image, tag)])
        .status()
        .await?;
    anyhow::ensure!(status.success(), "podman pull failed for {}:{}", image, tag);
    Ok(())
}

/// Stream logs for a container.
pub async fn logs(name: &str, follow: bool) -> Result<tokio::process::Child> {
    let mut cmd = tokio::process::Command::new("podman");
    cmd.args(["logs", "--timestamps"]);
    if follow {
        cmd.arg("--follow");
    }
    cmd.arg(name);
    cmd.stdout(std::process::Stdio::piped());
    Ok(cmd.spawn()?)
}
