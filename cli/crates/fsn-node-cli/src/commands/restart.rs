use std::path::Path;
use anyhow::Result;
use fsn_container::SystemdManager;

/// Restart one or all FSN-managed services.
pub async fn run(_root: &Path, _project: Option<&Path>, service: Option<&str>) -> Result<()> {
    let systemd = SystemdManager::new();
    if let Some(name) = service {
        let unit = format!("{}.service", name);
        systemd.stop(&unit).await?;
        systemd.start(&unit).await?;
        println!("Restarted {}", name);
    } else {
        let units = fsn_deploy::observe::list_fsn_units(&systemd).await?;
        for unit in &units {
            let _ = systemd.stop(unit).await;
            let _ = systemd.start(unit).await;
            println!("Restarted {}", unit.trim_end_matches(".service"));
        }
    }
    Ok(())
}
