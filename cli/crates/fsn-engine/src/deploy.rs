// Deploy engine – reconciles desired state with actual state.
// Replaces deploy-stack.yml + deploy-module.yml + generate-quadlet.yml
//
// Algorithm:
//   1. Flatten all module instances (sub-modules before parents)
//   2. Write .network + .container + .env Quadlet files
//   3. systemctl --user daemon-reload  (once)
//   4. For each instance: enable + start service
//   5. Wait for health check
//   6. Write deployed version marker
//
// Undeploy:
//   1. systemctl --user stop + disable
//   2. Remove Quadlet files
//   3. daemon-reload
//   4. Remove version marker

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use fsn_core::{
    config::{ProjectConfig, VaultConfig},
    state::desired::{DesiredState, ModuleInstance},
};
use fsn_podman::systemd;
use tracing::{info, warn};

use crate::generate::{env as gen_env, quadlet as gen_quadlet};
use crate::health;
use crate::hooks::{self, HookContext};

/// Options for the deploy operation.
#[derive(Debug, Clone)]
pub struct DeployOpts {
    /// Where to write Quadlet files (default: ~/.config/containers/systemd/)
    pub quadlet_dir: PathBuf,

    /// Where to write version markers (default: ~/.local/share/fsn/deployed/)
    pub state_dir: PathBuf,

    /// Only generate files, do not start services.
    pub dry_run: bool,

    /// How long to wait for each service to become healthy.
    pub health_timeout: Duration,
}

impl DeployOpts {
    pub fn default_for_user() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        Self {
            quadlet_dir:    PathBuf::from(&home).join(".config/containers/systemd"),
            state_dir:      PathBuf::from(&home).join(".local/share/fsn/deployed"),
            dry_run:        false,
            health_timeout: Duration::from_secs(120),
        }
    }
}

/// Deploy (or reconcile) the full desired state.
/// Sub-modules are always started before their parents.
pub async fn deploy_all(
    desired:   &DesiredState,
    project:   &ProjectConfig,
    vault:     &VaultConfig,
    opts:      &DeployOpts,
    fsn_root:  &Path,
    data_root: &Path,
) -> Result<()> {
    std::fs::create_dir_all(&opts.quadlet_dir)?;
    std::fs::create_dir_all(&opts.state_dir)?;

    let network_name = project_network_name(&desired.project_name);

    // ── Phase 1: Write the project .network Quadlet ───────────────────────────
    let net_content = gen_quadlet::generate_network(&network_name, &desired.project_name);
    let net_path    = opts.quadlet_dir.join(format!("{}.network", &network_name));
    write_if_changed(&net_path, &net_content)?;

    // ── Phase 2: Write all .container + .env files ────────────────────────────
    let instances = flatten_instances(&desired.modules);
    for instance in &instances {
        write_quadlet_files(instance, &network_name, opts)?;
    }

    if opts.dry_run {
        info!("Dry run: Quadlet files written, skipping systemd operations.");
        return Ok(());
    }

    // ── Phase 3: Reload systemd (once, after all files are on disk) ───────────
    info!("Reloading systemd user daemon…");
    systemd::daemon_reload().await?;

    // ── Phase 4: Enable + start + health check (sub-modules first) ───────────
    for instance in &instances {
        let unit = format!("{}.service", instance.name);
        info!("Starting {}…", instance.name);

        systemd::enable(&unit).await
            .with_context(|| format!("enabling {unit}"))?;
        systemd::start(&unit).await
            .with_context(|| format!("starting {unit}"))?;

        health::wait_for_ready(instance, opts.health_timeout).await
            .with_context(|| format!("health check for {}", instance.name))?;

        write_version_marker(instance, opts)?;

        // Post-deploy hook (idempotent: creates data dirs, renders configs, inits admin)
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

        info!("  ✓ {} running", instance.name);
    }

    Ok(())
}

/// Stop and remove a single service (keep data directories).
pub async fn undeploy_instance(name: &str, opts: &DeployOpts) -> Result<()> {
    let unit = format!("{}.service", name);

    // Best-effort stop/disable (may already be stopped)
    let _ = systemd::stop(&unit).await;
    let _ = run_systemctl_disable(&unit).await;

    // Remove Quadlet files
    let container_file = opts.quadlet_dir.join(format!("{}.container", name));
    let env_file       = opts.quadlet_dir.join(format!("{}.env", name));
    for f in [&container_file, &env_file] {
        if f.exists() { std::fs::remove_file(f)?; }
    }

    // Remove version marker
    let marker = opts.state_dir.join(format!("{}.version", name));
    if marker.exists() { std::fs::remove_file(marker)?; }

    systemd::daemon_reload().await?;
    info!("Removed {}", name);
    Ok(())
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Flatten instances into a list where sub-modules come before their parents.
pub fn flatten_instances(modules: &[ModuleInstance]) -> Vec<&ModuleInstance> {
    let mut out = Vec::new();
    for m in modules {
        flatten_recursive(m, &mut out);
    }
    out
}

fn flatten_recursive<'a>(instance: &'a ModuleInstance, out: &mut Vec<&'a ModuleInstance>) {
    // Sub-modules first (database, cache before the app)
    for sub in &instance.sub_modules {
        flatten_recursive(sub, out);
    }
    out.push(instance);
}

fn write_quadlet_files(
    instance:       &ModuleInstance,
    network_name:   &str,
    opts:           &DeployOpts,
) -> Result<()> {
    // .container
    let quadlet = gen_quadlet::generate(instance, Some(network_name))?;
    let qpath   = opts.quadlet_dir.join(format!("{}.container", instance.name));
    write_if_changed(&qpath, &quadlet)?;

    // .env
    let env_content = gen_env::generate(instance)?;
    let epath       = opts.quadlet_dir.join(format!("{}.env", instance.name));
    write_if_changed(&epath, &env_content)?;

    Ok(())
}

fn write_if_changed(path: &Path, content: &str) -> Result<()> {
    if path.exists() {
        let existing = std::fs::read_to_string(path)?;
        if existing == content {
            return Ok(()); // no change, skip write
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)
        .with_context(|| format!("writing {}", path.display()))
}

fn write_version_marker(instance: &ModuleInstance, opts: &DeployOpts) -> Result<()> {
    let path = opts.state_dir.join(format!("{}.version", instance.name));
    std::fs::write(&path, &instance.version)
        .with_context(|| format!("writing version marker {}", path.display()))
}

/// project_name → "fsn-myproject" (lowercase, hyphens)
pub fn project_network_name(project_name: &str) -> String {
    let slug: String = project_name
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    format!("fsn-{}", slug)
}

async fn run_systemctl_disable(unit: &str) -> Result<()> {
    let st = tokio::process::Command::new("systemctl")
        .args(["--user", "disable", unit])
        .status()
        .await?;
    anyhow::ensure!(st.success(), "systemctl --user disable {unit} failed");
    Ok(())
}
