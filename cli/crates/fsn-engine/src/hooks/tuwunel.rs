// Tuwunel (Matrix server) post-deploy hook.
// Replaces modules/chat/tuwunel/playbooks/deploy-setup.yml
//
// Tuwunel is a pure-Rust Matrix homeserver (formerly Conduit fork).
// Configuration is minimal: most settings come from environment variables,
// and `tuwunel.toml` supplements them.
//
// On first deploy:
//   1. Create data/ directory
//   2. Render tuwunel.toml.j2 if the template exists
//   3. Print helpful hints (federation, registration tokens)

use anyhow::Result;
use tracing::info;

use super::{common, HookContext};

pub async fn run(ctx: &HookContext<'_>) -> Result<()> {
    let data_dir = ctx.instance_data_dir();
    let name     = &ctx.instance.name;
    let domain   = &ctx.project.project.domain;

    common::create_dir(&data_dir.join("data"), 0o755)?;

    if !ctx.is_initialized() {
        // Render tuwunel.toml if template exists
        let config_dest = data_dir.join("data/tuwunel.toml");
        if ctx.templates_dir().join("tuwunel.toml.j2").exists() {
            if let Err(e) = common::write_template(ctx, "tuwunel.toml.j2", &config_dest) {
                tracing::warn!("{}: could not render tuwunel.toml.j2: {:#}", name, e);
            } else {
                info!("{}: tuwunel.toml written", name);
            }
        }

        ctx.mark_initialized()?;

        eprintln!();
        eprintln!("━━━  Tuwunel (Matrix)  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("Server ID:    {domain}");
        eprintln!("Client URL:   https://matrix.{domain}");
        eprintln!("Federation:   https://matrix.{domain}");
        eprintln!();
        eprintln!("Well-known records (must be served by Zentinel / reverse proxy):");
        eprintln!("  /.well-known/matrix/client  → {{\"m.homeserver\":{{\"base_url\":\"https://matrix.{domain}\"}}}}");
        eprintln!("  /.well-known/matrix/server  → {{\"m.server\":\"matrix.{domain}:443\"}}");
        eprintln!();
        eprintln!("To create the first admin user:");
        eprintln!("  podman exec -it {name} /usr/bin/tuwunel create-account --admin \\");
        eprintln!("    --username admin --password <pass> --server-name {domain}");
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!();
    }

    Ok(())
}
