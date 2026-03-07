// `fsn server setup` – prepare a Linux server for FreeSynergy.Node.
//
// Replaces playbooks/setup-server.yml
//
// What it does (must be run as root or via sudo):
//   1. Verify Podman ≥ 5.0 is installed
//   2. Ensure the deploy user exists (default: current user)
//   3. Enable systemd linger so user services survive logouts
//   4. Lower unprivileged port start to 80 (net.ipv4.ip_unprivileged_port_start=80)
//   5. Print a summary

use anyhow::{bail, Context, Result};
use std::path::Path;
use tracing::info;

const MIN_PODMAN_MAJOR: u32 = 5;

pub async fn run(root: &Path) -> Result<()> {
    check_root()?;

    let user = detect_deploy_user();

    // ── 1. Podman ─────────────────────────────────────────────────────────────
    let podman_ver = check_podman().await?;

    // ── 2. Deploy user ────────────────────────────────────────────────────────
    ensure_user(&user).await?;

    // ── 3. Linger ─────────────────────────────────────────────────────────────
    enable_linger(&user).await?;

    // ── 4. Unprivileged ports ─────────────────────────────────────────────────
    set_unprivileged_port_start().await?;

    // ── Summary ───────────────────────────────────────────────────────────────
    println!();
    println!("━━━  FreeSynergy.Node – Server Setup Complete  ━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  Deploy user:          {user}");
    println!("  Podman:               {podman_ver}");
    println!("  Linger:               enabled");
    println!("  Unprivileged ports:   from 80");
    println!();
    println!("Next step:  su - {user}  &&  fsn init");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn check_root() -> Result<()> {
    // nix would add a dependency; a simple uid check is enough
    let uid = unsafe { libc::getuid() };
    if uid != 0 {
        bail!("fsn server setup must be run as root (current uid: {})", uid);
    }
    Ok(())
}

fn detect_deploy_user() -> String {
    // If already running as a non-root user (via sudo), use SUDO_USER
    std::env::var("SUDO_USER")
        .ok()
        .filter(|u| !u.is_empty() && u != "root")
        .unwrap_or_else(|| "fsn".to_string())
}

/// Verify Podman ≥ 5.0 is installed and return version string.
async fn check_podman() -> Result<String> {
    let out = tokio::process::Command::new("podman")
        .arg("--version")
        .output()
        .await
        .context("podman not found – install Podman 5+ first")?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    // "podman version 5.4.1"
    let version = stdout.split_whitespace().last().unwrap_or("?").to_string();

    let major: u32 = version
        .split('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if major < MIN_PODMAN_MAJOR {
        bail!(
            "Podman {version} is too old (minimum: {MIN_PODMAN_MAJOR}.0). \
             Please upgrade: https://podman.io/getting-started/installation"
        );
    }

    info!("Podman {version} detected – OK");
    Ok(version)
}

/// Create the deploy user if they don't already exist.
async fn ensure_user(user: &str) -> Result<()> {
    // Check if user exists
    let st = tokio::process::Command::new("id")
        .arg(user)
        .status()
        .await?;

    if st.success() {
        info!("User '{user}' already exists – skipping creation");
        return Ok(());
    }

    info!("Creating user '{user}'…");
    let st = tokio::process::Command::new("useradd")
        .args(["--create-home", "--shell", "/bin/bash", user])
        .status()
        .await?;

    anyhow::ensure!(st.success(), "useradd {user} failed");
    println!("  Created user '{user}'");
    Ok(())
}

/// `loginctl enable-linger <user>` so systemd user services survive logout.
async fn enable_linger(user: &str) -> Result<()> {
    info!("Enabling linger for '{user}'…");
    let st = tokio::process::Command::new("loginctl")
        .args(["enable-linger", user])
        .status()
        .await
        .context("loginctl not found – is systemd installed?")?;

    anyhow::ensure!(st.success(), "loginctl enable-linger {user} failed");
    Ok(())
}

/// Set net.ipv4.ip_unprivileged_port_start=80 so rootless Podman can bind :80/:443.
async fn set_unprivileged_port_start() -> Result<()> {
    let conf_file = "/etc/sysctl.d/99-fsn-unprivileged-ports.conf";

    // Write sysctl config (persists across reboots)
    std::fs::write(conf_file, "net.ipv4.ip_unprivileged_port_start = 80\n")
        .with_context(|| format!("writing {conf_file}"))?;

    // Apply immediately
    let st = tokio::process::Command::new("sysctl")
        .args(["--system"])
        .status()
        .await
        .context("sysctl not found")?;

    anyhow::ensure!(st.success(), "sysctl --system failed");
    info!("net.ipv4.ip_unprivileged_port_start set to 80");
    Ok(())
}
