// Background deploy thread.
//
// Phase 1: Compose export (distributable templates for Docker/Podman Compose)
// Phase 2: Quadlet generation (systemd units for local Podman deployment)
//
// Both phases run in a single background thread — no async needed
// since all I/O is synchronous (registry scanning, file writes).

use std::path::Path;

use fsn_core::config::{HostConfig, resolve_plugins_dir};

use crate::app::{AppState, DeployMsg, DeployState, OverlayLayer};
use crate::handles::ProjectHandle;

/// Spawn a background deploy thread for the given project.
///
/// Accepts a full `ProjectHandle` (slug + config in one object) instead of
/// separate fields — the caller already has the handle, so there's no reason
/// to destructure it just to pass individual pieces.
pub fn trigger_deploy(
    state:    &mut AppState,
    root:     &Path,
    project:  ProjectHandle,
    host_cfg: Option<HostConfig>,
) {
    let (tx, rx) = std::sync::mpsc::channel::<DeployMsg>();
    state.deploy_rx = Some(rx);
    state.push_overlay(OverlayLayer::Deploy(DeployState {
        target:  project.config.project.meta.name.clone(),
        log:     Vec::new(),
        done:    false,
        success: false,
    }));

    let project_dir = root.join("projects").join(&project.slug);
    let modules_dir = resolve_plugins_dir(root);
    let project_cfg = project.config;

    std::thread::spawn(move || {
        // ── Phase 1: Compose export ───────────────────────────────────────────
        let compose_dir = project_dir.join("compose");
        let _ = tx.send(DeployMsg::Log("── Compose-Export ──".into()));

        if let Err(e) = std::fs::create_dir_all(&compose_dir) {
            let _ = tx.send(DeployMsg::Done { success: false, error: Some(e.to_string()) });
            return;
        }

        let compose_content = fsn_engine::generate::compose::generate_compose(&project_cfg);
        if let Err(e) = std::fs::write(compose_dir.join("compose.yml"), &compose_content) {
            let _ = tx.send(DeployMsg::Done { success: false, error: Some(format!("compose.yml: {e}")) });
            return;
        }
        let _ = tx.send(DeployMsg::Log("✓ compose/compose.yml".into()));

        let env_content = fsn_engine::generate::compose::generate_env_example(&project_cfg);
        if let Err(e) = std::fs::write(compose_dir.join(".env.example"), &env_content) {
            let _ = tx.send(DeployMsg::Done { success: false, error: Some(format!(".env.example: {e}")) });
            return;
        }
        let _ = tx.send(DeployMsg::Log("✓ compose/.env.example".into()));

        // ── Phase 2: Quadlet generation ───────────────────────────────────────
        let _ = tx.send(DeployMsg::Log("── Quadlet-Generierung ──".into()));

        let registry = match fsn_core::config::ServiceRegistry::load(&modules_dir) {
            Ok(r)  => r,
            Err(e) => {
                let _ = tx.send(DeployMsg::Log(format!("✗ Registry: {e}")));
                let _ = tx.send(DeployMsg::Done {
                    success: false,
                    error:   Some("Failed to load module registry".into()),
                });
                return;
            }
        };

        // Without a host config we can't resolve desired state — skip Quadlets.
        let host = match host_cfg {
            Some(h) => h,
            None => {
                let _ = tx.send(DeployMsg::Log("! No host configured — Quadlets skipped".into()));
                let _ = tx.send(DeployMsg::Log("  → Please add a host first (Sidebar → n)".into()));
                let _ = tx.send(DeployMsg::Done { success: true, error: None });
                return;
            }
        };

        let vault = fsn_core::config::VaultConfig::load(&project_dir, None)
            .unwrap_or_default();

        let data_root = project_dir.join("data");
        let desired = match fsn_engine::resolve::resolve_desired(
            &project_cfg, &host, &registry, &vault, Some(&data_root),
        ) {
            Ok(d)  => d,
            Err(e) => {
                let _ = tx.send(DeployMsg::Done { success: false, error: Some(format!("Resolve: {e}")) });
                return;
            }
        };

        let quadlet_dir = project_dir.join("quadlets");
        if let Err(e) = std::fs::create_dir_all(&quadlet_dir) {
            let _ = tx.send(DeployMsg::Done { success: false, error: Some(e.to_string()) });
            return;
        }

        let network_name = format!(
            "fsn-{}",
            project_cfg.project.meta.name.to_lowercase().replace(' ', "-")
        );

        let net_content = fsn_engine::generate::quadlet::generate_network(
            &network_name, &project_cfg.project.meta.name,
        );
        if let Err(e) = std::fs::write(quadlet_dir.join(format!("{network_name}.network")), &net_content) {
            let _ = tx.send(DeployMsg::Log(format!("✗ {network_name}.network: {e}")));
        } else {
            let _ = tx.send(DeployMsg::Log(format!("✓ {network_name}.network")));
        }

        // Flatten all instances: sub-services must be written before their parent
        // so that systemd ordering directives (After=) refer to existing units.
        let mut all_instances = Vec::new();
        for svc in &desired.services {
            for sub in &svc.sub_services { all_instances.push(sub); }
            all_instances.push(svc);
        }

        for instance in &all_instances {
            match fsn_engine::generate::quadlet::generate(instance, Some(&network_name)) {
                Ok(content) => {
                    let fname = format!("{}.container", instance.name);
                    if let Err(e) = std::fs::write(quadlet_dir.join(&fname), &content) {
                        let _ = tx.send(DeployMsg::Log(format!("✗ {fname}: {e}")));
                    } else {
                        let _ = tx.send(DeployMsg::Log(format!("✓ {fname}")));
                    }

                    let env_fname = format!("{}.env", instance.name);
                    let env_lines: String = instance.resolved_env.iter()
                        .map(|(k, v)| format!("{k}={v}\n"))
                        .collect();
                    if let Err(e) = std::fs::write(quadlet_dir.join(&env_fname), &env_lines) {
                        let _ = tx.send(DeployMsg::Log(format!("✗ {env_fname}: {e}")));
                    } else {
                        let _ = tx.send(DeployMsg::Log(format!("✓ {env_fname}")));
                    }
                }
                Err(e) => {
                    let _ = tx.send(DeployMsg::Log(format!("✗ {}: {e}", instance.name)));
                }
            }
        }

        let _ = tx.send(DeployMsg::Done { success: true, error: None });
    });
}
