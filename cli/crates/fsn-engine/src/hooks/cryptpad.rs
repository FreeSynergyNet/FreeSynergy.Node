// CryptPad post-deploy hook.
// Replaces modules/collab/cryptpad/playbooks/deploy-setup.yml
//
// CryptPad runs as UID 4001 inside the container.
// We need:
//   data/blob/        – encrypted file chunks
//   data/blobstage/   – upload staging
//   data/block/       – user account blocks
//   data/datastore/   – main data
//   data/customize/   – optional theme overrides
//
// Config is done through environment variables (injected by .env Quadlet),
// so there is no template to render by default.
// If a config.js.j2 or sso.js.j2 template exists, render it.

use anyhow::Result;
use tracing::info;

use super::{common, HookContext};

/// UID:GID used by CryptPad inside the container.

pub async fn run(ctx: &HookContext<'_>) -> Result<()> {
    let data_dir = ctx.instance_data_dir();
    let name     = &ctx.instance.name;

    // ── Directories (must be writable by UID 4001 inside the container) ───────
    //
    // We can't chown inside the container from the host without root (or newuidmap),
    // so we create them world-writable during setup; tighter perms can be set later
    // via `podman exec cryptpad chown -R cryptpad:cryptpad /cryptpad/data`.
    for subdir in &["blob", "blobstage", "block", "datastore", "customize"] {
        let path = data_dir.join("data").join(subdir);
        common::create_dir(&path, 0o1777)?;  // sticky world-writable
    }
    info!("{}: data directories created", name);

    if !ctx.is_initialized() {
        // Optional: render config.js if template exists
        let config_dest = data_dir.join("data/customize/config.js");
        if ctx.templates_dir().join("config.js.j2").exists() && !config_dest.exists() {
            if let Err(e) = common::write_template(ctx, "config.js.j2", &config_dest) {
                tracing::warn!("{}: could not render config.js.j2: {:#}", name, e);
            } else {
                info!("{}: config.js rendered", name);
            }
        }

        // Optional: render SSO config if template exists
        let sso_dest = data_dir.join("data/customize/sso.js");
        if ctx.templates_dir().join("sso.js.j2").exists() && !sso_dest.exists() {
            if let Err(e) = common::write_template(ctx, "sso.js.j2", &sso_dest) {
                tracing::warn!("{}: could not render sso.js.j2: {:#}", name, e);
            } else {
                info!("{}: sso.js rendered", name);
            }
        }

        ctx.mark_initialized()?;
        info!(
            "{}: ready. Login at https://{}  (admin configured via environment)",
            name, ctx.instance.service_domain
        );
    }

    Ok(())
}
