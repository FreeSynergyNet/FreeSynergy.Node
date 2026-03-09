// System information collector.
// Reads hostname, current user, primary IP, RAM, CPU count, uptime,
// Podman version, and system architecture.

use std::process::Command;
use sysinfo::System;

#[derive(Debug, Clone)]
pub struct SysInfo {
    pub hostname:       String,
    pub user:           String,
    pub ip:             String,
    pub ram_used_gb:    f64,
    pub ram_total_gb:   f64,
    pub cpu_cores:      usize,
    pub uptime_str:     String,
    pub podman_version: String,
    pub arch:           String,
}

impl Default for SysInfo {
    fn default() -> Self {
        Self {
            hostname: "test-host".into(), user: "test".into(), ip: "127.0.0.1".into(),
            ram_used_gb: 0.0, ram_total_gb: 16.0, cpu_cores: 4,
            uptime_str: "0m".into(), podman_version: "n/a".into(),
            arch: "x86_64".into(),
        }
    }
}

impl SysInfo {
    pub fn collect() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let ram_total_gb = sys.total_memory() as f64 / 1_073_741_824.0;
        let ram_used_gb  = sys.used_memory()  as f64 / 1_073_741_824.0;
        let cpu_cores    = sys.cpus().len();
        let uptime_secs  = System::uptime();
        let uptime_str   = format_uptime(uptime_secs);

        SysInfo {
            hostname:       System::host_name().unwrap_or_else(|| "unknown".into()),
            user:           whoami(),
            ip:             primary_ip(),
            ram_used_gb,
            ram_total_gb,
            cpu_cores,
            uptime_str,
            podman_version: podman_version(),
            arch:           std::env::consts::ARCH.to_string(),
        }
    }

    /// Formatted RAM string: "4.2 / 16.0 GB"
    pub fn ram_str(&self) -> String {
        format!("{:.1} / {:.1} GB", self.ram_used_gb, self.ram_total_gb)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn whoami() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

fn primary_ip() -> String {
    // Parse `hostname -I` — first token is the primary IP
    Command::new("hostname")
        .arg("-I")
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).to_string();
            s.split_whitespace().next().map(|ip| ip.to_string())
        })
        .unwrap_or_else(|| "n/a".into())
}

fn podman_version() -> String {
    Command::new("podman")
        .args(["version", "--format", "{{.Client.Version}}"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })
        .unwrap_or_else(|| "n/a".into())
}

fn format_uptime(secs: u64) -> String {
    let days  = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins  = (secs % 3600) / 60;
    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}
