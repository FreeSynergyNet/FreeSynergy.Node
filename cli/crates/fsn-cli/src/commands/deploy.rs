// fsn deploy – reconcile desired state and start/update services.
// Replaces: ansible-playbook playbooks/deploy-stack.yml

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use fsn_core::{
    config::{HostConfig, ModuleRegistry, ProjectConfig, VaultConfig},
};
use fsn_engine::{
    deploy::{DeployOpts, deploy_all},
    diff::compute_diff,
    observe::observe,
    resolve::resolve_desired,
};
use tracing::info;

pub async fn run(
    root:       &Path,
    project:    Option<&Path>,
    service:    Option<&str>,
) -> Result<()> {
    // ── Load configs ──────────────────────────────────────────────────────────
    let project_path = find_project(root, project)
        .context("No project file found. Run `fsn init` first.")?;
    let proj = ProjectConfig::load(&project_path)?;

    let host_path = find_host(root)
        .context("No host file found. Run `fsn init` first.")?;
    let host = HostConfig::load(&host_path)?;

    let vault_pass = std::env::var("FSN_VAULT_PASS").ok();
    let vault = VaultConfig::load(
        project_path.parent().unwrap_or(root),
        vault_pass.as_deref(),
    )?;

    let registry = ModuleRegistry::load(&root.join("modules"))?;

    // ── Resolve desired state ─────────────────────────────────────────────────
    let desired = resolve_desired(&proj, &host, &registry, &vault)
        .context("Resolving desired state")?;

    // ── Observe actual state ──────────────────────────────────────────────────
    let actual = observe().await?;

    // ── Compute diff ──────────────────────────────────────────────────────────
    let diff = compute_diff(&desired, &actual);

    if diff.is_empty() && service.is_none() {
        println!("Nothing to do – all services are already up to date.");
        return Ok(());
    }

    info!("Deploy plan: {}", diff.summary());

    // Filter to a single service if requested
    let deploy_desired = if let Some(svc) = service {
        use fsn_core::state::DesiredState;
        let modules = desired.modules.into_iter()
            .filter(|m| m.name == svc || m.sub_modules.iter().any(|s| s.name == svc))
            .collect();
        DesiredState { modules, ..desired }
    } else {
        desired
    };

    // ── Deploy ────────────────────────────────────────────────────────────────
    let opts      = DeployOpts::default_for_user();
    let data_root = project_path.parent()
        .map(|p| p.join("data"))
        .unwrap_or_else(|| root.join("data"));

    deploy_all(&deploy_desired, &proj, &vault, &opts, root, &data_root).await
        .context("Deploy failed")?;

    println!("\n✓ Deploy complete ({} service(s))", deploy_desired.modules.len());
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub(crate) fn find_project(root: &Path, explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit { return Some(p.to_path_buf()); }
    let projects = root.join("projects");
    std::fs::read_dir(&projects).ok()?.flatten()
        .filter(|e| e.path().is_dir())
        .flat_map(|d| std::fs::read_dir(d.path()).into_iter().flatten().flatten())
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|e| e.to_str()) == Some("toml")
              && p.to_string_lossy().contains(".project."))
}

pub(crate) fn find_host(root: &Path) -> Option<PathBuf> {
    let hosts = root.join("hosts");
    std::fs::read_dir(&hosts).ok()?.flatten()
        .map(|e| e.path())
        .find(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            p.extension().and_then(|e| e.to_str()) == Some("toml")
                && name.ends_with(".host.toml")
                && name != "example.host.toml"
        })
}
