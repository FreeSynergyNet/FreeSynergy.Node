use std::path::Path;
use anyhow::Result;
use fsn_engine::deploy::{DeployOpts, undeploy_instance};
use fsn_podman::systemd;

pub async fn run(root: &Path, _project: Option<&Path>, service: Option<&str>) -> Result<()> {
    let opts = DeployOpts::default_for_user();
    if let Some(name) = service {
        undeploy_instance(name, &opts).await?;
        println!("Undeployed {}", name);
    } else {
        let units = systemd::list_fsn_units().await?;
        for unit in &units {
            let name = unit.trim_end_matches(".service");
            undeploy_instance(name, &opts).await?;
            println!("Undeployed {}", name);
        }
    }
    Ok(())
}
