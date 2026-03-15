// Lifecycle hook executor.
//
// Runs [lifecycle] hooks declared in module TOML files.
//
// Supported actions per phase:
//   on_install      → run, bus_emit
//   on_peer_install → run (triggered when another service is installed)
//   on_update       → backup, run
//   on_swap         → export, run
//   on_decommission → backup, run
//
// All hooks are best-effort: failures are logged as warnings but do not
// abort the deploy. The lifecycle system must never block service start.

use anyhow::Result;
use tracing::{info, warn};

use fsn_node_core::config::service::{LifecycleAction, LifecycleHook, PeerHook};

use super::HookContext;
use super::common::podman_exec;

// ── Public entry points ───────────────────────────────────────────────────────

/// Run `on_install` hooks for the given instance.
pub async fn run_on_install(ctx: &HookContext<'_>) -> Result<()> {
    let hooks = ctx.instance.class.lifecycle.on_install.clone();
    if hooks.is_empty() { return Ok(()); }
    info!("[lifecycle] {} on_install: {} hook(s)", ctx.instance.name, hooks.len());
    for hook in &hooks {
        if let Err(e) = run_hook(ctx, hook).await {
            warn!("[lifecycle] {} on_install hook failed (continuing): {:#}", ctx.instance.name, e);
        }
    }
    Ok(())
}

/// Run `on_update` hooks for the given instance.
pub async fn run_on_update(ctx: &HookContext<'_>) -> Result<()> {
    let hooks = ctx.instance.class.lifecycle.on_update.clone();
    if hooks.is_empty() { return Ok(()); }
    info!("[lifecycle] {} on_update: {} hook(s)", ctx.instance.name, hooks.len());
    for hook in &hooks {
        if let Err(e) = run_hook(ctx, hook).await {
            warn!("[lifecycle] {} on_update hook failed (continuing): {:#}", ctx.instance.name, e);
        }
    }
    Ok(())
}

/// Run `on_decommission` hooks for the given instance.
pub async fn run_on_decommission(ctx: &HookContext<'_>) -> Result<()> {
    let hooks = ctx.instance.class.lifecycle.on_decommission.clone();
    if hooks.is_empty() { return Ok(()); }
    info!("[lifecycle] {} on_decommission: {} hook(s)", ctx.instance.name, hooks.len());
    for hook in &hooks {
        if let Err(e) = run_hook(ctx, hook).await {
            warn!("[lifecycle] {} on_decommission hook failed (continuing): {:#}", ctx.instance.name, e);
        }
    }
    Ok(())
}

/// Run `on_swap` hooks for the given instance (this service is being replaced).
pub async fn run_on_swap(ctx: &HookContext<'_>) -> Result<()> {
    let hooks = ctx.instance.class.lifecycle.on_swap.clone();
    if hooks.is_empty() { return Ok(()); }
    info!("[lifecycle] {} on_swap: {} hook(s)", ctx.instance.name, hooks.len());
    for hook in &hooks {
        if let Err(e) = run_hook(ctx, hook).await {
            warn!("[lifecycle] {} on_swap hook failed (continuing): {:#}", ctx.instance.name, e);
        }
    }
    Ok(())
}

// ── Internal hook executors ───────────────────────────────────────────────────

async fn run_hook(ctx: &HookContext<'_>, hook: &LifecycleHook) -> Result<()> {
    match hook.action {
        LifecycleAction::Run => run_shell(ctx, hook.command.as_deref()).await,
        LifecycleAction::BusEmit => {
            if let Some(event) = &hook.event {
                info!("[lifecycle:bus_emit] {} → event={}", ctx.instance.name, event);
                // Bus integration placeholder — bus_emit will be wired in Teil 6
                // when fsn-bus is available as a dependency.
            }
            Ok(())
        }
        LifecycleAction::Backup => run_backup(ctx, hook.target.as_deref()).await,
        LifecycleAction::Export => run_export(ctx, hook.target.as_deref(), hook.format.as_deref()).await,
    }
}

/// Public wrapper for `on_peer_install` hook execution, called from deploy.rs.
pub async fn run_peer_hook_pub(ctx: &HookContext<'_>, hook: &PeerHook) -> Result<()> {
    run_peer_hook(ctx, hook).await
}

async fn run_peer_hook(ctx: &HookContext<'_>, hook: &PeerHook) -> Result<()> {
    match hook.action {
        LifecycleAction::Run => run_shell(ctx, hook.command.as_deref()).await,
        LifecycleAction::BusEmit => Ok(()),   // peer hooks don't emit
        LifecycleAction::Backup  => Ok(()),   // peer hooks don't backup
        LifecycleAction::Export  => Ok(()),   // peer hooks don't export
    }
}

/// Run a shell command inside the container via `podman exec`.
/// Command is split on spaces (no shell expansion — use scripts for complex logic).
async fn run_shell(ctx: &HookContext<'_>, command: Option<&str>) -> Result<()> {
    let cmd = match command {
        Some(c) if !c.trim().is_empty() => c,
        _ => {
            warn!("[lifecycle:run] {} has no command", ctx.instance.name);
            return Ok(());
        }
    };

    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let (bin, args) = parts.split_first().unwrap_or((&"", &[]));

    info!("[lifecycle:run] {} exec: {}", ctx.instance.name, cmd);

    let out = podman_exec(&ctx.instance.name, &{
        let mut all = vec![*bin];
        all.extend_from_slice(args);
        all
    })
    .await?;

    if !out.is_empty() {
        info!("[lifecycle:run] {} output: {}", ctx.instance.name, out.trim());
    }
    Ok(())
}

/// Create a backup of the instance data directory.
async fn run_backup(ctx: &HookContext<'_>, target: Option<&str>) -> Result<()> {
    let src = ctx.instance_data_dir();
    let ts  = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dst = target
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| src.with_file_name(format!(
            "{}-backup-{}",
            ctx.instance.name, ts
        )));

    info!("[lifecycle:backup] {} → {}", src.display(), dst.display());

    // Use `cp -a` for a recursive copy preserving permissions.
    let status = tokio::process::Command::new("cp")
        .args(["-a", &src.to_string_lossy(), &dst.to_string_lossy()])
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("backup cp failed for {}", ctx.instance.name);
    }
    Ok(())
}

/// Export instance data to a portable format.
/// Currently only "json" format is supported (via container exec).
async fn run_export(ctx: &HookContext<'_>, target: Option<&str>, format: Option<&str>) -> Result<()> {
    let fmt = format.unwrap_or("json");
    let out_path = target
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(format!(
            "/tmp/fsn-export-{}.{}",
            ctx.instance.name, fmt
        )));

    info!("[lifecycle:export] {} format={} → {}", ctx.instance.name, fmt, out_path.display());
    // Actual export implementation is service-specific and provided via
    // the `command` field in the hook (run_shell handles the exec).
    // This stub logs intent and returns OK for Bus-signalling purposes.
    Ok(())
}
