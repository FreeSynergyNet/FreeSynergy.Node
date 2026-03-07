use std::path::Path;
use anyhow::Result;
use fsn_podman::systemd;

pub async fn run(_root: &Path, _project: Option<&Path>, service: Option<&str>) -> Result<()> {
    if let Some(name) = service {
        let unit = format!("{}.service", name);
        systemd::stop(&unit).await?;
        systemd::start(&unit).await?;
        println!("Restarted {}", name);
    } else {
        let units = systemd::list_fsn_units().await?;
        for unit in &units {
            let _ = systemd::stop(unit).await;
            let _ = systemd::start(unit).await;
            println!("Restarted {}", unit.trim_end_matches(".service"));
        }
    }
    Ok(())
}
