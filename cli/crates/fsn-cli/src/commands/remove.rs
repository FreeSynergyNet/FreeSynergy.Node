use std::path::Path;
use anyhow::{bail, Result};
use fsn_engine::deploy::{DeployOpts, undeploy_instance};
use fsn_podman::systemd;

pub async fn run(root: &Path, project: Option<&Path>, service: Option<&str>, confirm: bool) -> Result<()> {
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
        let units = systemd::list_fsn_units().await?;
        for unit in &units {
            let name = unit.trim_end_matches(".service");
            undeploy_instance(name, &opts).await?;
            println!("Removed {}", name);
        }
    }
    Ok(())
}
