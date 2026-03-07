// Forgejo post-deploy hook.
// Replaces modules/git/forgejo/playbooks/deploy-setup.yml

use anyhow::Result;
use tracing::info;

use super::{common, HookContext};

pub async fn run(ctx: &HookContext<'_>) -> Result<()> {
    let data_dir = ctx.instance_data_dir();
    let name     = &ctx.instance.name;

    common::create_dir(&data_dir.join("data"), 0o755)?;

    if !ctx.is_initialized() {
        info!("{}: creating admin user…", name);

        let admin_user  = "fsn-admin";
        let admin_email = format!("admin@{}", ctx.project.project.domain);
        let admin_pass  = ctx.vault.expose("vault_forgejo_admin_password")
            .unwrap_or("changeme-please-set-vault_forgejo_admin_password");

        let _ = common::podman_exec(name, &[
            "forgejo", "admin", "user", "create",
            "--admin",
            "--username",               admin_user,
            "--password",               admin_pass,
            "--email",                  &admin_email,
            "--must-change-password=false",
        ]).await;

        ctx.mark_initialized()?;
        info!("{}: admin user '{}' created", name, admin_user);
    }

    // OIDC source (always try – idempotent via "already exists" check)
    if ctx.desired.modules.iter()
        .any(|m| m.class_key == "auth/kanidm" || m.sub_modules.iter().any(|s| s.class_key == "auth/kanidm"))
    {
        let client_secret = ctx.vault.expose("vault_forgejo_oidc_client_secret")
            .unwrap_or("");
        if !client_secret.is_empty() {
            // Try to find the kanidm service domain
            let kanidm_domain = ctx.desired.modules.iter()
                .find(|m| m.class_key == "auth/kanidm")
                .map(|m| m.service_domain.clone())
                .unwrap_or_else(|| format!("kanidm.{}", ctx.project.project.domain));

            let discover_url = format!(
                "https://{}/oauth2/openid/forgejo/.well-known/openid-configuration",
                kanidm_domain
            );

            let _ = common::podman_exec(name, &[
                "forgejo", "admin", "auth", "add-oauth",
                "--name",            "kanidm",
                "--provider",        "openidConnect",
                "--key",             "forgejo",
                "--secret",          client_secret,
                "--auto-discover-url", &discover_url,
                "--scopes",          "openid email profile groups",
                "--auto-register",
            ]).await;

            info!("{}: OIDC source 'kanidm' configured", name);
        } else {
            tracing::warn!(
                "{}: vault_forgejo_oidc_client_secret not set – OIDC not configured",
                name
            );
        }
    }

    Ok(())
}
