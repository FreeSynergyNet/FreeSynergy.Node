use std::path::Path;
use anyhow::Result;
use fsn_container::SystemdManager;

/// Print the systemd state of all FSN-managed services.
pub async fn run(_root: &Path, _project: Option<&Path>) -> Result<()> {
    let systemd = SystemdManager::new();
    let units = fsn_deploy::observe::list_fsn_units(&systemd).await?;

    if units.is_empty() {
        println!("{}", fsn_i18n::t("status.no-services"));
        return Ok(());
    }

    println!("{:<30} {}", fsn_i18n::t("status.header-service"), fsn_i18n::t("status.header-state"));
    println!("{}", "─".repeat(42));

    for unit in &units {
        let name = unit.trim_end_matches(".service");
        let state = match systemd.status(unit).await {
            Ok(s)  => s.active_state.to_string(),
            Err(_) => "error".to_string(),
        };
        println!("{:<30} {}", name, state);
    }

    Ok(())
}
