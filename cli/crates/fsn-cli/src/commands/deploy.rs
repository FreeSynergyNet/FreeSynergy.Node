// fsn deploy – reconcile desired state and start/update services.
// Replaces: ansible-playbook playbooks/deploy-stack.yml

use std::path::Path;

use anyhow::{Context, Result};
use fsn_core::{
    config::{HostConfig, ServiceRegistry, ProjectConfig, VaultConfig, resolve_plugins_dir,
             find_project, find_host, find_host_by_name},
};
use fsn_deploy::{
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
    target_host: Option<&str>,
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

    let registry = ServiceRegistry::load(&resolve_plugins_dir(root))?;

    // ── Resolve desired state ─────────────────────────────────────────────────
    let data_root = project_path.parent()
        .map(|p| p.join("data"))
        .unwrap_or_else(|| root.join("data"));
    let desired = resolve_desired(&proj, &host, &registry, &vault, Some(&data_root))
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
        let services = desired.services.into_iter()
            .filter(|m| m.name == svc || m.sub_services.iter().any(|s| s.name == svc))
            .collect();
        DesiredState { services, ..desired }
    } else {
        desired
    };

    // ── Build DeployOpts (local or remote) ───────────────────────────────────
    let mut opts = DeployOpts::default_for_user();

    if let Some(host_name) = target_host {
        let remote = build_remote_host(root, host_name)
            .with_context(|| format!("Host '{host_name}' not found. Check your *.host.toml files."))?;
        opts.remote_host = Some(remote);
    }

    deploy_all(&deploy_desired, &proj, &vault, &opts, root, &data_root).await
        .context("Deploy failed")?;

    crate::db::write_audit_entry(
        &fsn_core::audit::AuditEntry::new("system", "deploy", "project", &proj.project.meta.name),
    ).await;

    println!("\n✓ Deploy complete ({} service(s))", deploy_desired.services.len());
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a RemoteHost from a host config file matched by name.
fn build_remote_host(root: &Path, host_name: &str) -> Option<fsn_host::RemoteHost> {
    let host_path = find_host_by_name(root, host_name)?;
    let cfg = HostConfig::load(&host_path).ok()?;
    let h = &cfg.host;
    Some(fsn_host::RemoteHost {
        name:         h.meta.name.clone(),
        address:      h.addr().to_string(),
        ssh_port:     h.ssh_port,
        ssh_user:     h.ssh_user.clone(),
        ssh_key_path: h.ssh_key_path.clone(),
    })
}
