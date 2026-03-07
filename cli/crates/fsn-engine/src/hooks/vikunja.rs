// Vikunja post-deploy hook.
// Replaces modules/tasks/vikunja/playbooks/deploy-setup.yml
//
// Vikunja needs:
//   data/files/  – uploaded attachments (owned by UID 1000 in the container)
//
// On first deploy we also print the URL and explain that the first registered
// user becomes the admin (Vikunja default behaviour).

use anyhow::Result;
use tracing::info;

use super::{common, HookContext};

pub async fn run(ctx: &HookContext<'_>) -> Result<()> {
    let data_dir = ctx.instance_data_dir();
    let name     = &ctx.instance.name;

    // Vikunja runs as UID 1000 inside the container.
    // Use world-writable + sticky so both host and container can write.
    common::create_dir(&data_dir.join("data"),       0o755)?;
    common::create_dir(&data_dir.join("data/files"), 0o1777)?;

    if !ctx.is_initialized() {
        ctx.mark_initialized()?;

        eprintln!();
        eprintln!("━━━  Vikunja  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("URL: https://{}", ctx.instance.service_domain);
        eprintln!();
        eprintln!("First registration creates the admin account.");
        eprintln!("To disable open registration afterwards:");
        eprintln!("  set VIKUNJA_SERVICE_ENABLEREGISTRATION=false in the environment.");
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!();
        info!("{}: data/files directory ready", name);
    }

    Ok(())
}
