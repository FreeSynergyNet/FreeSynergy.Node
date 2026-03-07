// Kanidm post-deploy hook.
//
// Replaces modules/auth/kanidm/playbooks/deploy-setup.yml
//
// On first deploy (no .initialized marker):
//   1. Create data directories (data/, data/backups/)
//   2. Render server.toml from template
//   3. Recover admin + idm_admin accounts → save credentials
//   4. Mark as initialized
//
// On every deploy (idempotent OAuth2 registration):
//   5. For every module that uses kanidm as a service (OIDC clients),
//      ensure the OAuth2 client is registered

use anyhow::{Context, Result};
use tracing::info;

use super::{common, HookContext};

pub async fn run(ctx: &HookContext<'_>) -> Result<()> {
    let data_dir = ctx.instance_data_dir();
    let name     = &ctx.instance.name;

    // ── 1. Directories ────────────────────────────────────────────────────────
    common::create_dir(&data_dir.join("data"),          0o755)?;
    common::create_dir(&data_dir.join("data/backups"),  0o755)?;

    // ── 2. Render server.toml ─────────────────────────────────────────────────
    let server_toml = data_dir.join("data/server.toml");
    if !server_toml.exists() {
        // Only write on first run; admin may have customised it
        if let Ok(content) = super::common::render_template(ctx, "kanidm.toml.j2") {
            std::fs::write(&server_toml, content)
                .with_context(|| format!("writing {}", server_toml.display()))?;
        } else {
            tracing::warn!("{}: template kanidm.toml.j2 not found – skipping", name);
        }
    }

    // ── 3. First-time admin initialisation ────────────────────────────────────
    if !ctx.is_initialized() {
        info!("{}: recovering admin accounts…", name);

        let admin_pw = common::podman_exec(name, &[
            "kanidmd", "recover-account", "admin", "-c", "/data/server.toml",
        ]).await.unwrap_or_else(|_| "(recovery failed – check kanidm logs)".into());

        let idm_pw = common::podman_exec(name, &[
            "kanidmd", "recover-account", "idm_admin", "-c", "/data/server.toml",
        ]).await.unwrap_or_else(|_| "(recovery failed)".into());

        let creds_path = data_dir.join("data/.admin-credentials");
        let content = format!(
            "# Kanidm initial credentials – KEEP SECRET\n\
             # Change via: kanidm account change-password --name admin\n\
             admin: {}\nidm_admin: {}\n",
            admin_pw, idm_pw
        );
        std::fs::write(&creds_path, &content)?;
        std::fs::set_permissions(&creds_path,
            std::os::unix::fs::PermissionsExt::from_mode(0o600))?;

        ctx.mark_initialized()?;
        info!("{}: admin credentials saved to {}", name, creds_path.display());
    }

    // ── 4. OAuth2 client registration (every deploy, idempotent) ─────────────
    register_oauth2_clients(ctx).await?;

    Ok(())
}

/// Register OAuth2 clients in Kanidm for every module that uses Kanidm OIDC.
async fn register_oauth2_clients(ctx: &HookContext<'_>) -> Result<()> {
    let name    = &ctx.instance.name;
    let _domain = &ctx.project.project.domain;

    // Collect all instances (and sub-modules) that load kanidm as a service
    let oidc_clients = collect_oidc_clients(ctx);

    for client in oidc_clients {
        let client_id    = &client.name;
        let display_name = &client.class.module.name;
        let origin       = format!("https://{}", client.service_domain);

        info!("{}: registering OAuth2 client '{}'…", name, client_id);

        // create (ignore "already exists")
        let _ = common::podman_exec(name, &[
            "kanidm", "system", "oauth2", "create",
            client_id, display_name, &origin,
            "--name", "idm_admin",
        ]).await;

        // scope map
        let _ = common::podman_exec(name, &[
            "kanidm", "system", "oauth2", "update-scope-map",
            client_id, "idm_all_accounts",
            "openid", "email", "profile",
            "--name", "idm_admin",
        ]).await;

        // prefer short username (more user-friendly in apps)
        let _ = common::podman_exec(name, &[
            "kanidm", "system", "oauth2", "prefer-short-username",
            client_id, "--name", "idm_admin",
        ]).await;
    }

    Ok(())
}

/// Find all instances that have kanidm as a declared service dependency.
fn collect_oidc_clients<'a>(
    ctx: &HookContext<'a>,
) -> Vec<&'a fsn_core::state::desired::ModuleInstance> {
    let mut out = Vec::new();
    collect_recursive(&ctx.desired.modules, &mut out);
    // Exclude kanidm itself
    out.retain(|inst| inst.class_key != "auth/kanidm");
    out
}

fn collect_recursive<'a>(
    modules: &'a [fsn_core::state::desired::ModuleInstance],
    out: &mut Vec<&'a fsn_core::state::desired::ModuleInstance>,
) {
    for m in modules {
        if m.class.load.services.contains_key("kanidm") {
            out.push(m);
        }
        collect_recursive(&m.sub_modules, out);
    }
}
