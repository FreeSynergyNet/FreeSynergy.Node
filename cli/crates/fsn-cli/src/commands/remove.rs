use std::path::Path;
use anyhow::{bail, Result};
use fsn_deploy::deploy::{DeployOpts, undeploy_all, undeploy_instance};

/// Remove one or all deployed services (stops units, deletes Quadlet files).
pub async fn run(_root: &Path, _project: Option<&Path>, service: Option<&str>, confirm: bool) -> Result<()> {
    if !confirm {
        bail!(
            "Remove deletes ALL data for {}. Re-run with --confirm to proceed.",
            service.unwrap_or("all services")
        );
    }
    let opts = DeployOpts::default_for_user();
    if let Some(name) = service {
        undeploy_instance(name, &opts).await?;
        println!("Removed {}", name);
    } else {
        let n = undeploy_all(&opts).await?;
        println!("Removed {} service(s).", n);
    }
    Ok(())
}
