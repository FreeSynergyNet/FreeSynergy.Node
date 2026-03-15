use std::path::Path;
use anyhow::Result;

// Update = deploy with force flag (pulls latest image, restarts changed services).
// Delegates to deploy::run for now; image pulling is handled by Podman on restart.
pub async fn run(root: &Path, project: Option<&Path>, service: Option<&str>) -> Result<()> {
    crate::commands::deploy::run(root, project, service, None).await
}
