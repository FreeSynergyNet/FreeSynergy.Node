// `fsn tui` — launches FreeSynergy.Desktop (fsd / fsd-conductor).
//
// Search order:
//   1. `fsd-conductor` in PATH  — standalone conductor binary (if built separately)
//   2. `fsd` in PATH            — full Desktop shell (includes conductor as a window)
//   3. Well-known build locations for both binaries
//
// fsd-conductor is the container management app (service list, logs, health status).
// fsd is the full Desktop shell that hosts conductor and other apps.

use std::path::Path;
use anyhow::{bail, Result};

pub async fn run(_root: &Path) -> Result<()> {
    if let Some(bin) = which_desktop_bin() {
        eprintln!("Starting FreeSynergy.Desktop ({})…", bin.display());
        let status = std::process::Command::new(&bin)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to launch {}: {e}", bin.display()))?;

        if !status.success() {
            bail!("{} exited with status {status}", bin.display());
        }
        Ok(())
    } else {
        eprintln!("FreeSynergy.Desktop (fsd / fsd-conductor) not found in PATH.");
        eprintln!("Build it with:");
        eprintln!("  cd /home/kal/Server/FreeSynergy.Desktop");
        eprintln!("  cargo build -p fsd-app --release");
        eprintln!("  sudo cp target/release/fsd /usr/local/bin/fsd");
        bail!("fsd not installed")
    }
}

/// Find the best available Desktop binary.
///
/// Prefers `fsd-conductor` (standalone conductor mode) over the full `fsd`
/// shell, so that `fsn tui` opens container management directly.
/// Falls back to `fsd` if conductor is not separately installed.
fn which_desktop_bin() -> Option<std::path::PathBuf> {
    // Check PATH for both candidates (conductor first)
    for name in &["fsd-conductor", "fsd"] {
        if let Ok(out) = std::process::Command::new("which").arg(name).output() {
            if out.status.success() {
                let p = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    // Check well-known local build locations (development fallback)
    let home = std::env::var("HOME").unwrap_or_default();
    let base = format!("{home}/Server/FreeSynergy.Desktop/target");
    let candidates = [
        format!("{base}/release/fsd-conductor"),
        format!("{base}/debug/fsd-conductor"),
        format!("{base}/release/fsd"),
        format!("{base}/debug/fsd"),
    ];
    candidates.iter()
        .map(std::path::PathBuf::from)
        .find(|p| p.exists())
}
