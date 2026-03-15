//! Remote deploy — write Quadlet files and manage systemd via SSH.
//!
//! This module mirrors the local deploy path but uses [`SshSession`] for all
//! file I/O and [`RemoteSystemd`] for service control.

use std::path::Path;

use anyhow::{Context, Result};
use fsn_node_core::{
    config::{ProjectConfig, VaultConfig},
    state::desired::{DesiredState, ServiceInstance},
};
use fsn_host::{RemoteHost, RemoteSystemd, SshSession};
use tracing::{info, warn};

use crate::{
    deploy::{flatten_instances, project_network_name},
    generate::{env as gen_env, quadlet as gen_quadlet},
    health,
    hooks::{self, HookContext},
    module_runner::{ContextBuilder, ModuleRunner},
};
use super::deploy::DeployOpts;

/// Remote-deploy the full desired state to `host` via SSH.
///
/// Quadlet files are written to the user quadlet directory on the remote host,
/// then systemd is reloaded and services are started remotely.
/// Health checks still run from the local machine over HTTP.
pub async fn deploy_all_remote(
    desired:    &DesiredState,
    project:    &ProjectConfig,
    vault:      &VaultConfig,
    opts:       &DeployOpts,
    fsn_root:   &Path,
    data_root:  &Path,
    remote:     &RemoteHost,
) -> Result<()> {
    info!("Remote deploy → {}", remote.address);
    let session = SshSession::connect(remote).await
        .with_context(|| format!("SSH connect to {}", remote.address))?;
    let systemd = RemoteSystemd::new(&session);

    let network_name = project_network_name(&desired.project_name);
    let quadlet_dir  = opts.quadlet_dir.to_string_lossy();

    // ── Phase 1: Ensure remote quadlet directory exists ───────────────────────
    session.exec(&format!("mkdir -p {quadlet_dir}")).await?.into_result()
        .context("create remote quadlet dir")?;

    // ── Phase 2: Write .network Quadlet ───────────────────────────────────────
    let net_content = gen_quadlet::generate_network(&network_name, &desired.project_name);
    let net_path    = format!("{quadlet_dir}/{network_name}.network");
    session.write_file(&net_path, net_content.as_bytes()).await
        .with_context(|| format!("writing {net_path}"))?;

    // ── Phase 3: Write all .container + .env Quadlet files ───────────────────
    let instances = flatten_instances(&desired.services);
    for instance in &instances {
        write_remote_quadlet_files(instance, &network_name, &quadlet_dir, &session).await?;
    }

    if opts.dry_run {
        info!("Dry run: remote files written, skipping systemd operations.");
        session.close().await.ok();
        return Ok(());
    }

    // ── Phase 4: Reload remote systemd ────────────────────────────────────────
    systemd.daemon_reload().await
        .context("remote systemd daemon-reload failed")?;

    // ── Phase 5: Enable + start + health check ────────────────────────────────
    for instance in &instances {
        let unit = format!("{}.service", instance.name);
        info!("Remote start {}…", instance.name);

        let _ = systemd.enable(&unit).await;
        systemd.start(&unit).await
            .with_context(|| format!("remote start {unit}"))?;

        // Health check runs from local machine (HTTP to remote host IP)
        health::wait_for_ready(instance, opts.health_timeout).await
            .with_context(|| format!("health check for {}", instance.name))?;

        // Write local version marker
        write_version_marker_local(instance, opts)?;

        // Post-deploy hook (runs locally, generates config files, etc.)
        let hook_ctx = HookContext {
            instance,
            desired,
            project,
            vault,
            data_root: data_root.to_path_buf(),
            fsn_root,
        };
        if let Err(e) = hooks::run_hook(&hook_ctx).await {
            warn!("  hook for {} failed: {:#}", instance.name, e);
        }

        info!("  ✓ {} running (remote)", instance.name);
    }

    // ── Phase 6: Run plugin generate-config ───────────────────────────────────
    run_remote_plugin_configs(desired, data_root, opts, &session).await?;

    session.close().await.ok();
    Ok(())
}

// ── Internal helpers ──────────────────────────────────────────────────────────

async fn write_remote_quadlet_files(
    instance:     &ServiceInstance,
    network_name: &str,
    quadlet_dir:  &str,
    session:      &SshSession,
) -> Result<()> {
    // .container
    let quadlet = gen_quadlet::generate(instance, Some(network_name))?;
    let qpath   = format!("{quadlet_dir}/{}.container", instance.name);
    session.write_file(&qpath, quadlet.as_bytes()).await
        .with_context(|| format!("writing remote {qpath}"))?;

    // .env
    let env_content = gen_env::generate(instance)?;
    let epath = format!("{quadlet_dir}/{}.env", instance.name);
    session.write_file(&epath, env_content.as_bytes()).await
        .with_context(|| format!("writing remote {epath}"))?;

    Ok(())
}

fn write_version_marker_local(instance: &ServiceInstance, opts: &DeployOpts) -> Result<()> {
    std::fs::create_dir_all(&opts.state_dir)?;
    let path = opts.state_dir.join(format!("{}.version", instance.name));
    std::fs::write(&path, &instance.version)
        .with_context(|| format!("writing version marker {}", path.display()))
}

async fn run_remote_plugin_configs(
    desired:   &DesiredState,
    data_root: &Path,
    opts:      &DeployOpts,
    session:   &SshSession,
) -> Result<()> {
    for instance in &desired.services {
        let has_generate_config = instance.class.manifest.as_ref()
            .map(|m| m.commands.iter().any(|c| c == "generate-config"))
            .unwrap_or(false);

        if !has_generate_config {
            continue;
        }

        let Some(store_root) = &opts.store_root else {
            warn!("  Skipping remote generate-config for '{}': no store_root set", instance.name);
            continue;
        };

        let store_module_dir = store_root.join(&instance.class_key);
        let runner = ModuleRunner::new(&store_module_dir);

        let peers: Vec<&ServiceInstance> = desired.services.iter()
            .filter(|s| s.name != instance.name)
            .collect();

        let data_root_str = data_root.join(&instance.name).to_string_lossy().into_owned();
        let ctx = ContextBuilder::build(
            "generate-config",
            instance,
            &desired.domain,
            &data_root_str,
            &peers,
        );

        let response = runner.run(&ctx)
            .with_context(|| format!("plugin generate-config for '{}'", instance.name))?;

        // For remote: upload generated output files via SSH
        for output in &response.files {
            session.write_file(&output.dest, output.content.as_bytes()).await
                .with_context(|| format!("writing remote plugin output {}", output.dest))?;
            info!("  ✓ {} remote config written → {}", instance.name, output.dest);
        }

        for log in &response.logs {
            info!("  [{}] {}", instance.name, log.message);
        }
    }
    Ok(())
}
