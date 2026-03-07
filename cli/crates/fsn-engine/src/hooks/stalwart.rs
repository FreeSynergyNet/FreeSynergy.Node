// Stalwart Mail post-deploy hook.
// Replaces modules/mail/stalwart/playbooks/deploy-setup.yml
//
// On first deploy:
//   1. Create data/ directory
//   2. Extract the auto-generated admin password from container logs
//   3. Save credentials to data/.admin-credentials
//   4. Print DNS records that need to be added
//
// Stalwart prints something like:
//   "Administrator password: <RANDOM>"  in its initial startup logs.

use anyhow::Result;
use tracing::{info, warn};

use super::{common, HookContext};

pub async fn run(ctx: &HookContext<'_>) -> Result<()> {
    let data_dir = ctx.instance_data_dir();
    let name     = &ctx.instance.name;
    let domain   = &ctx.project.project.domain;

    common::create_dir(&data_dir.join("data"), 0o755)?;

    if !ctx.is_initialized() {
        info!("{}: reading initial admin password from logs…", name);

        // Stalwart logs the admin password during first startup
        let logs = common::podman_logs_tail(name, 100).await
            .unwrap_or_default();

        let admin_pass = extract_admin_password(&logs)
            .unwrap_or_else(|| {
                warn!("{}: could not find admin password in logs – check manually", name);
                "(see container logs)".to_string()
            });

        let creds_path = data_dir.join("data/.admin-credentials");
        let content = format!(
            "# Stalwart Mail initial credentials – KEEP SECRET\n\
             # Web UI: https://mail.{domain}/admin\n\
             # username: admin\n\
             # password: {admin_pass}\n\
             \n\
             # Change via the web UI after first login.\n"
        );
        std::fs::write(&creds_path, &content)?;
        std::fs::set_permissions(
            &creds_path,
            std::os::unix::fs::PermissionsExt::from_mode(0o600),
        )?;

        ctx.mark_initialized()?;

        info!("{}: credentials saved to {}", name, creds_path.display());
        print_dns_hints(domain);
    }

    Ok(())
}

/// Parse "Administrator password: <PW>" from Stalwart startup logs.
fn extract_admin_password(logs: &str) -> Option<String> {
    for line in logs.lines() {
        // Stalwart ≥ 0.6 prints "Administrator account 'admin' is ready with password '<PW>'"
        // Older: "Generated admin password: <PW>"
        for prefix in &[
            "password '",
            "Generated admin password: ",
            "Administrator password: ",
        ] {
            if let Some(idx) = line.find(prefix) {
                let rest = &line[idx + prefix.len()..];
                let pw = rest.trim_end_matches('\'').trim_end_matches('"').trim();
                if !pw.is_empty() {
                    return Some(pw.to_string());
                }
            }
        }
    }
    None
}

/// Print the DNS records that must be created for mail delivery.
fn print_dns_hints(domain: &str) {
    eprintln!();
    eprintln!("━━━  Stalwart DNS records  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("Add these records to your DNS zone ({domain}):");
    eprintln!();
    eprintln!("  MX  {domain}.                      10  mail.{domain}.");
    eprintln!("  A   mail.{domain}.                     <server-ip>");
    eprintln!();
    eprintln!("  TXT {domain}.                      \"v=spf1 mx ~all\"");
    eprintln!("  TXT _dmarc.{domain}.               \"v=DMARC1; p=quarantine; rua=mailto:dmarc@{domain}\"");
    eprintln!();
    eprintln!("  # DKIM key will be shown in the Stalwart admin UI after first login.");
    eprintln!("  # TXT mail._domainkey.{domain}.   \"v=DKIM1; k=rsa; p=<key>\"");
    eprintln!();
    eprintln!("  SRV _submission._tcp.{domain}.  10 1 587 mail.{domain}.");
    eprintln!("  SRV _imaps._tcp.{domain}.        10 1 993 mail.{domain}.");
    eprintln!("  SRV _jmap._tcp.{domain}.          10 1 443 mail.{domain}.");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!();
}
