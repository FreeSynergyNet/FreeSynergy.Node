// fsn sync – show what would change without applying anything.
// Replaces: ansible-playbook playbooks/sync-stack.yml

use std::path::Path;
use anyhow::Result;
use fsn_core::config::{HostConfig, ModuleRegistry, ProjectConfig, VaultConfig};
use fsn_engine::{diff::compute_diff, observe::observe, resolve::resolve_desired};

use crate::commands::deploy::{find_project, find_host};

pub async fn run(root: &Path, project: Option<&Path>) -> Result<()> {
    let proj_path = find_project(root, project).ok_or_else(|| anyhow::anyhow!("No project file found"))?;
    let host_path = find_host(root).ok_or_else(|| anyhow::anyhow!("No host file found"))?;
    let proj      = ProjectConfig::load(&proj_path)?;
    let host      = HostConfig::load(&host_path)?;
    let vault_pass = std::env::var("FSN_VAULT_PASS").ok();
    let vault = VaultConfig::load(proj_path.parent().unwrap_or(root), vault_pass.as_deref())?;
    let registry  = ModuleRegistry::load(&root.join("modules"))?;
    let desired   = resolve_desired(&proj, &host, &registry, &vault)?;
    let actual    = observe().await?;
    let diff      = compute_diff(&desired, &actual);

    if diff.is_empty() {
        println!("✓ All services are up to date.");
        return Ok(());
    }

    if !diff.to_deploy.is_empty() {
        println!("To deploy ({}):", diff.to_deploy.len());
        for m in &diff.to_deploy { println!("  + {}", m.name); }
    }
    if !diff.to_update.is_empty() {
        println!("To update ({}):", diff.to_update.len());
        for m in &diff.to_update { println!("  ~ {}", m.name); }
    }
    if !diff.to_remove.is_empty() {
        println!("To remove ({}):", diff.to_remove.len());
        for n in &diff.to_remove { println!("  - {}", n); }
    }
    Ok(())
}
