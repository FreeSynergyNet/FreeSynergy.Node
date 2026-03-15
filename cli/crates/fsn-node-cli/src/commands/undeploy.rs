use std::path::Path;
use anyhow::Result;
use fsn_deploy::deploy::{DeployOpts, undeploy_all, undeploy_instance};

/// Stop and remove Quadlet files for one or all services.
pub async fn run(_root: &Path, _project: Option<&Path>, service: Option<&str>) -> Result<()> {
    let opts = DeployOpts::default_for_user();
    if let Some(name) = service {
        undeploy_instance(name, &opts).await?;
        println!("Undeployed {}", name);
    } else {
        let n = undeploy_all(&opts).await?;
        println!("Undeployed {} service(s).", n);
    }
    Ok(())
}
