use std::path::Path;
use anyhow::Result;

pub async fn run(_root: &Path, _project: Option<&Path>) -> Result<()> {
    // podman system prune removes stopped containers, dangling images, unused networks
    let st = tokio::process::Command::new("podman")
        .args(["system", "prune", "--force"])
        .status()
        .await?;
    anyhow::ensure!(st.success(), "podman system prune failed");
    println!("Cleaned up stopped containers and dangling images.");
    Ok(())
}
