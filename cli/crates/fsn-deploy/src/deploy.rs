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
//   7. Run plugin generate-config for every service that declares it
//      (plugin path when store_root set; built-in fallback for proxy)
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
    config::{ProjectConfig, VaultConfig, service::ServiceType},
    state::desired::{DesiredState, ServiceInstance},
};
use fsn_container::SystemdManager;
use tracing::{info, warn};

use crate::generate::{env as gen_env, kdl as gen_kdl, quadlet as gen_quadlet};
use crate::health;
use crate::hooks::{self, HookContext};
use crate::module_runner::{ContextBuilder, ModuleRunner};
use crate::remote;

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

    /// Local root of the synced Store module tree
    /// (e.g. `~/.local/share/fsn/store/fsn-official/Node/`).
    ///
    /// When set, services with a `[plugin]` manifest and a `generate-config`
    /// command are invoked via the process plugin protocol.
    /// When absent, built-in generators are used as fallback.
    pub store_root: Option<PathBuf>,

    /// When set, deploy to this remote host via SSH instead of running locally.
    pub remote_host: Option<fsn_host::RemoteHost>,
}

impl DeployOpts {
    pub fn default_for_user() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        Self {
            quadlet_dir:    PathBuf::from(&home).join(".config/containers/systemd"),
            state_dir:      PathBuf::from(&home).join(".local/share/fsn/deployed"),
            dry_run:        false,
            health_timeout: Duration::from_secs(120),
            store_root:     None,
            remote_host:    None,
        }
    }
}

/// Deploy (or reconcile) the full desired state.
/// Sub-modules are always started before their parents.
/// When `opts.remote_host` is set, deploys via SSH instead of running locally.
pub async fn deploy_all(
    desired:   &DesiredState,
    project:   &ProjectConfig,
    vault:     &VaultConfig,
    opts:      &DeployOpts,
    fsn_root:  &Path,
    data_root: &Path,
) -> Result<()> {
    // Dispatch to remote path when a target host is configured
    if let Some(host) = &opts.remote_host {
        return remote::deploy_all_remote(desired, project, vault, opts, fsn_root, data_root, host).await;
    }

    std::fs::create_dir_all(&opts.quadlet_dir)?;
    std::fs::create_dir_all(&opts.state_dir)?;

    let network_name = project_network_name(&desired.project_name);

    // ── Phase 1: Write the project .network Quadlet ───────────────────────────
    let net_content = gen_quadlet::generate_network(&network_name, &desired.project_name);
    let net_path    = opts.quadlet_dir.join(format!("{}.network", &network_name));
    write_if_changed(&net_path, &net_content)?;

    // ── Phase 2: Write all .container + .env files ────────────────────────────
    let instances = flatten_instances(&desired.services);
    for instance in &instances {
        write_quadlet_files(instance, &network_name, opts)?;
    }

    if opts.dry_run {
        info!("Dry run: Quadlet files written, skipping systemd operations.");
        return Ok(());
    }

    // ── Phase 3: Reload systemd (once, after all files are on disk) ───────────
    info!("Reloading systemd user daemon…");
    let systemd = SystemdManager::new();
    systemd.daemon_reload().await
        .with_context(|| "systemd daemon-reload failed")?;

    // ── Phase 4: Enable + start + health check (sub-modules first) ───────────
    for instance in &instances {
        let unit = format!("{}.service", instance.name);
        info!("Starting {}…", instance.name);

        // Quadlet-generated units are auto-enabled via WantedBy=default.target
        // during daemon-reload — calling enable separately is not needed and
        // will fail with "unit is transient or generated". Best-effort only.
        let _ = systemd.enable(&unit).await;
        systemd.start(&unit).await
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

    // ── Phase 5: Run plugin generate-config for all applicable services ───────
    run_all_plugin_configs(desired, data_root, opts)?;

    Ok(())
}

/// Stop and remove all FSN-managed services.
///
/// Returns the number of services that were undeployed.
pub async fn undeploy_all(opts: &DeployOpts) -> Result<usize> {
    let systemd = SystemdManager::new();
    let units = crate::observe::list_fsn_units(&systemd).await?;
    for unit in &units {
        let name = unit.trim_end_matches(".service");
        undeploy_instance(name, opts).await?;
    }
    Ok(units.len())
}

/// Stop and remove a single service (keep data directories).
pub async fn undeploy_instance(name: &str, opts: &DeployOpts) -> Result<()> {
    let unit = format!("{}.service", name);
    let systemd = SystemdManager::new();

    // Best-effort stop/disable (may already be stopped)
    let _ = systemd.stop(&unit).await;
    let _ = systemd.disable(&unit).await;

    // Remove Quadlet files
    let container_file = opts.quadlet_dir.join(format!("{}.container", name));
    let env_file       = opts.quadlet_dir.join(format!("{}.env", name));
    for f in [&container_file, &env_file] {
        if f.exists() { std::fs::remove_file(f)?; }
    }

    // Remove version marker
    let marker = opts.state_dir.join(format!("{}.version", name));
    if marker.exists() { std::fs::remove_file(marker)?; }

    systemd.daemon_reload().await
        .with_context(|| "systemd daemon-reload failed")?;
    info!("Removed {}", name);
    Ok(())
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Flatten instances into a list where sub-modules come before their parents.
pub fn flatten_instances(modules: &[ServiceInstance]) -> Vec<&ServiceInstance> {
    let mut out = Vec::new();
    for m in modules {
        flatten_recursive(m, &mut out);
    }
    out
}

fn flatten_recursive<'a>(instance: &'a ServiceInstance, out: &mut Vec<&'a ServiceInstance>) {
    // Sub-modules first (database, cache before the app)
    for sub in &instance.sub_services {
        flatten_recursive(sub, out);
    }
    out.push(instance);
}

fn write_quadlet_files(
    instance:       &ServiceInstance,
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

fn write_version_marker(instance: &ServiceInstance, opts: &DeployOpts) -> Result<()> {
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

// ── Plugin config generation (Phase 5) ───────────────────────────────────────

/// Run `generate-config` for every top-level service that declares it.
///
/// Per service, two paths:
///   Plugin  — service has `[plugin]` with `generate-config` + `store_root` is set
///   Builtin — service is a Proxy and no plugin path available (no `store_root`
///             or no manifest); falls back to `gen_kdl`
/// Run `generate-config` for all applicable services.
///
/// Plugin failures are non-fatal: a warning is emitted and the rest of the
/// deploy continues.  This implements graceful degradation — a broken or
/// missing plugin must never block unrelated services from starting.
fn run_all_plugin_configs(
    desired:   &DesiredState,
    data_root: &Path,
    opts:      &DeployOpts,
) -> Result<()> {
    for instance in &desired.services {
        if let Err(e) = run_service_plugin_config(instance, desired, data_root, opts) {
            warn!(
                service = %instance.name,
                "plugin generate-config failed (continuing): {:#}", e
            );
        }
    }
    Ok(())
}

fn run_service_plugin_config(
    instance:  &ServiceInstance,
    desired:   &DesiredState,
    data_root: &Path,
    opts:      &DeployOpts,
) -> Result<()> {
    let has_generate_config = instance.class.manifest.as_ref()
        .map(|m| m.commands.iter().any(|c| c == "generate-config"))
        .unwrap_or(false);

    // Plugin path: manifest + store_root available
    if has_generate_config {
        if let Some(store_root) = &opts.store_root {
            return run_plugin_generate_config(instance, desired, data_root, store_root);
        }
    }

    // Built-in fallback: only proxy services have a built-in generator
    if instance.class.meta.has_type(&ServiceType::Proxy) {
        return write_zentinel_kdl_builtin(instance, desired, data_root);
    }

    // Other services without store_root: warn if they have a manifest, skip otherwise
    if has_generate_config {
        warn!(
            "  Skipping generate-config for '{}': no store_root set (set store_root in DeployOpts)",
            instance.name
        );
    }

    Ok(())
}

/// Invoke the plugin executable for `generate-config`.
fn run_plugin_generate_config(
    instance:   &ServiceInstance,
    desired:    &DesiredState,
    data_root:  &Path,
    store_root: &Path,
) -> Result<()> {
    // Store layout: {store_root}/{class_key}/  e.g. store_root/proxy/zentinel/
    let store_module_dir = store_root.join(&instance.class_key);
    let runner = ModuleRunner::new(&store_module_dir);

    // Peers = all services except this one
    let peers: Vec<&ServiceInstance> = desired
        .services
        .iter()
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

    for log in &response.logs {
        info!("  [{}] {}", instance.name, log.message);
    }

    runner.apply(&response)
        .with_context(|| format!("applying plugin output for '{}'", instance.name))?;

    info!("  ✓ {} config written (via plugin)", instance.name);
    Ok(())
}

/// Built-in Zentinel KDL generator — fallback when no store_root or no manifest.
///
/// - Existing file: only the FSN-managed block is replaced (markers preserved).
/// - New file: full config is generated (server + listeners + managed section).
fn write_zentinel_kdl_builtin(
    proxy:     &ServiceInstance,
    desired:   &DesiredState,
    data_root: &Path,
) -> Result<()> {
    let kdl_path = data_root
        .join(&proxy.name)
        .join("config")
        .join("zentinel.kdl");

    if let Some(parent) = kdl_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let new_content = if kdl_path.exists() {
        let existing = std::fs::read_to_string(&kdl_path)?;
        gen_kdl::upsert_managed_section(&existing, desired)
    } else {
        gen_kdl::generate_full_config(desired)
    };

    write_if_changed(&kdl_path, &new_content)?;
    info!("  ✓ Zentinel config written (built-in) → {}", kdl_path.display());

    Ok(())
}
