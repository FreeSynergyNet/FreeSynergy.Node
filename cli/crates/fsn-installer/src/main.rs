// FreeSynergy.Node – Bootstrap Installer
//
// Replaces fsn-install.sh.  Distributed as a pre-built binary; bootstrapped via:
//
//   curl -fsSL https://install.freesynergy.net/fsn-installer -o fsn-installer
//   chmod +x fsn-installer && ./fsn-installer
//
// Flags mirror the old shell script:
//   --repo URL       FSN repository to clone  (default: official GitHub)
//   --target DIR     Installation directory    (default: ~/FreeSynergy.Node)
//   --skip-build     Download pre-built binary instead of compiling
//   --skip-init      Clone + build only; skip `fsn init`

use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{bail, Context, Result};
use clap::Parser;

// ── CLI args ──────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(name = "fsn-installer", about = "FreeSynergy.Node – bootstrap installer")]
struct Args {
    /// FSN repository URL to clone
    #[arg(long, default_value = "https://github.com/FreeSynergy/Node")]
    repo: String,

    /// Installation directory
    #[arg(long)]
    target: Option<PathBuf>,

    /// Download a pre-built `fsn` binary instead of compiling from source
    #[arg(long)]
    skip_build: bool,

    /// Clone + build only; skip `fsn init`
    #[arg(long)]
    skip_init: bool,
}

// ── OS detection ──────────────────────────────────────────────────────────────

fn detect_os() -> String {
    if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if let Some(id) = line.strip_prefix("ID=") {
                return id.trim_matches('"').to_string();
            }
        }
    }
    "unknown".to_string()
}

// ── Print helpers ─────────────────────────────────────────────────────────────

fn info(msg: &str)  { eprintln!("\x1b[1;34m==> \x1b[0m{msg}"); }
fn ok(msg: &str)    { eprintln!("\x1b[1;32m✓   \x1b[0m{msg}"); }
fn warn(msg: &str)  { eprintln!("\x1b[1;33m!   \x1b[0m{msg}"); }

// ── System dependency check ───────────────────────────────────────────────────

fn install_deps(os: &str) -> Result<()> {
    let missing: Vec<&str> = ["git", "curl", "podman"]
        .into_iter()
        .filter(|cmd| which(cmd).is_none())
        .collect();

    if !std::path::Path::new("/run/systemd/system").exists() {
        bail!("systemd is required but not running. FSN uses Podman Quadlets (systemd user units).");
    }

    if missing.is_empty() {
        ok("All system dependencies present.");
        return Ok(());
    }

    info(&format!("Installing missing packages: {}", missing.join(" ")));

    let pkgs = missing.join(" ");
    let result = match os {
        "fedora" | "rhel" | "centos" | "rocky" | "almalinux" => {
            run_cmd("sudo", &["dnf", "install", "-y", &pkgs])
        }
        "debian" | "ubuntu" | "linuxmint" | "pop" => {
            run_cmd("sudo", &["apt-get", "install", "-y", &pkgs])
        }
        "arch" | "manjaro" => {
            run_cmd("sudo", &["pacman", "-Sy", "--noconfirm", &pkgs])
        }
        os if os.starts_with("opensuse") || os == "sles" => {
            run_cmd("sudo", &["zypper", "install", "-y", &pkgs])
        }
        _ => {
            warn(&format!("Unknown OS – please install manually: {pkgs}"));
            return Ok(());
        }
    };

    result.with_context(|| format!("installing packages: {pkgs}"))
}

fn which(cmd: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|dir| {
            let full = dir.join(cmd);
            full.is_file().then_some(full)
        })
    })
}

fn run_cmd(prog: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(prog)
        .args(args)
        .status()
        .with_context(|| format!("running {prog}"))?;
    anyhow::ensure!(status.success(), "{prog} exited with {status}");
    Ok(())
}

// ── Podman socket ─────────────────────────────────────────────────────────────

fn enable_podman_socket() {
    let active = Command::new("systemctl")
        .args(["--user", "is-active", "podman.socket"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !active {
        info("Enabling Podman user socket…");
        let _ = Command::new("systemctl")
            .args(["--user", "enable", "--now", "podman.socket"])
            .status();
    }

    // Enable lingering so user units survive logout
    if which("loginctl").is_some() {
        let user = env::var("USER").unwrap_or_default();
        let _ = Command::new("loginctl")
            .args(["enable-linger", &user])
            .stderr(Stdio::null())
            .status();
    }
}

// ── Clone / update repo ───────────────────────────────────────────────────────

fn ensure_repo(repo_url: &str, target: &Path) -> Result<()> {
    if target.join(".git").exists() {
        info(&format!("Updating existing repo at {}", target.display()));
        run_cmd("git", &["-C", &target.to_string_lossy(), "pull", "--ff-only"])
    } else {
        info(&format!("Cloning FreeSynergy.Node to {}", target.display()));
        std::fs::create_dir_all(target.parent().unwrap_or(target))?;
        run_cmd("git", &["clone", "--depth", "1", repo_url, &target.to_string_lossy()])
    }
}

// ── Build / download binary ───────────────────────────────────────────────────

fn build_binary(target_dir: &Path, bin_path: &Path) -> Result<()> {
    // Ensure rustup / cargo present
    if which("cargo").is_none() {
        info("Installing Rust toolchain via rustup…");
        let sh = reqwest_blocking_get("https://sh.rustup.rs")?;
        let tmp = std::env::temp_dir().join("rustup-init.sh");
        std::fs::write(&tmp, sh)?;
        run_cmd("sh", &[&tmp.to_string_lossy(), "--", "-y", "--profile", "minimal"])
            .context("installing rustup")?;

        // Source cargo env (best-effort)
        let cargo_env = dirs_home().join(".cargo").join("env");
        if cargo_env.exists() {
            // Can't `source` in Rust; update PATH manually
            let cargo_bin = dirs_home().join(".cargo").join("bin");
            let old_path = env::var("PATH").unwrap_or_default();
            env::set_var("PATH", format!("{}:{old_path}", cargo_bin.display()));
        }
    }

    info("Building fsn binary (this may take a few minutes on first run)…");
    let cli_dir = target_dir.join("cli");
    run_cmd(
        "cargo",
        &["build", "--release", "-p", "fsn-node-cli", "--manifest-path",
          &cli_dir.join("Cargo.toml").to_string_lossy()],
    ).context("cargo build")?;

    let built = cli_dir.join("target").join("release").join("fsn");
    std::fs::create_dir_all(bin_path.parent().unwrap_or(bin_path))?;
    std::fs::copy(&built, bin_path)
        .with_context(|| format!("copying fsn binary to {}", bin_path.display()))?;

    // Set executable bit
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(bin_path, std::fs::Permissions::from_mode(0o755))?;
    }

    ok(&format!("Installed fsn to {}", bin_path.display()));
    Ok(())
}

fn download_binary(repo_url: &str, bin_path: &Path) -> Result<()> {
    let arch = std::env::consts::ARCH;
    let url = format!("{repo_url}/releases/latest/download/fsn-{arch}-unknown-linux-musl");
    info(&format!("Downloading pre-built fsn binary from {url}…"));

    let bytes = reqwest_blocking_get(&url)?;
    std::fs::create_dir_all(bin_path.parent().unwrap_or(bin_path))?;
    std::fs::write(bin_path, bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(bin_path, std::fs::Permissions::from_mode(0o755))?;
    }

    ok(&format!("Downloaded fsn to {}", bin_path.display()));
    Ok(())
}

// ── HTTP helper (blocking via reqwest) ───────────────────────────────────────

fn reqwest_blocking_get(url: &str) -> Result<Vec<u8>> {
    // We're inside a Tokio runtime (main is async), use block_in_place.
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let client = reqwest::Client::builder()
                .https_only(true)
                .build()?;
            let resp = client.get(url).send().await?.error_for_status()?;
            Ok(resp.bytes().await?.to_vec())
        })
    })
}

fn dirs_home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

// ── main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    let target = args.target.unwrap_or_else(|| dirs_home().join("FreeSynergy.Node"));
    let bin_path = dirs_home().join(".local").join("bin").join("fsn");

    let os = detect_os();
    info(&format!("Detected OS: {os}"));

    install_deps(&os)?;
    enable_podman_socket();
    ensure_repo(&args.repo, &target)?;

    if args.skip_build {
        download_binary(&args.repo, &bin_path)?;
    } else {
        build_binary(&target, &bin_path)?;
    }

    // Ensure ~/.local/bin is on PATH for the fsn init call below
    let local_bin = dirs_home().join(".local").join("bin");
    let old_path = env::var("PATH").unwrap_or_default();
    env::set_var("PATH", format!("{}:{old_path}", local_bin.display()));

    // Verify binary runs
    match Command::new(&bin_path).arg("--version").output() {
        Ok(out) => {
            let ver = String::from_utf8_lossy(&out.stdout);
            ok(&format!("fsn {} ready.", ver.trim()));
        }
        Err(e) => bail!("installed fsn binary not executable: {e}"),
    }

    if !args.skip_init {
        info("Starting setup wizard…");
        let status = Command::new(&bin_path)
            .arg("init")
            .arg("--root")
            .arg(&target)
            .status()
            .context("running fsn init")?;
        if !status.success() {
            bail!("fsn init exited with {status}");
        }
    }

    Ok(())
}
