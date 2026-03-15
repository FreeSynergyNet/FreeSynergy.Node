// `fsn conductor` — container management via the Podman socket.
//
// Wraps PodmanClient (fsn-container) in a Conductor facade so that every
// operation shares a single client instance instead of constructing one
// per function call.
//
// For a graphical view, use `fsn tui` (opens fsd-conductor).

use anyhow::{bail, Result};
use fsn_container::PodmanClient;
use tokio::time::{sleep, Duration};

// ── Conductor (Facade) ────────────────────────────────────────────────────────

/// Facade over PodmanClient for direct container management from the CLI.
pub struct Conductor {
    client: PodmanClient,
}

impl Conductor {
    pub fn new() -> Result<Self> {
        Ok(Self { client: PodmanClient::new()? })
    }

    // ── start ─────────────────────────────────────────────────────────────────

    /// Start a stopped container by name.
    pub async fn start(&self, service: &str) -> Result<()> {
        self.client.start(service).await?;
        println!("Started: {service}");
        Ok(())
    }

    // ── stop ──────────────────────────────────────────────────────────────────

    /// Stop a running container by name.
    pub async fn stop(&self, service: &str) -> Result<()> {
        self.client.stop(service, None).await?;
        println!("Stopped: {service}");
        Ok(())
    }

    // ── restart ───────────────────────────────────────────────────────────────

    /// Restart a container by name.
    pub async fn restart(&self, service: &str) -> Result<()> {
        self.client.restart(service).await?;
        println!("Restarted: {service}");
        Ok(())
    }

    // ── logs ──────────────────────────────────────────────────────────────────

    /// Print recent log lines for a container.
    ///
    /// When `follow` is `true`, polls for new lines every second until interrupted.
    pub async fn logs(&self, service: &str, follow: bool, tail: u64) -> Result<()> {
        if self.client.inspect(service).await?.is_none() {
            bail!("container not found: {service}");
        }

        if !follow {
            let lines = self.client.logs(service, Some(tail)).await?;
            for line in lines {
                println!("{line}");
            }
            return Ok(());
        }

        // Follow mode: print initial batch then poll for new lines.
        let mut printed = 0usize;
        loop {
            let lines = self.client.logs(service, Some(tail.max(printed as u64 + 1))).await?;
            for line in lines.iter().skip(printed) {
                println!("{line}");
            }
            printed = lines.len();
            sleep(Duration::from_secs(1)).await;
        }
    }

    // ── list ──────────────────────────────────────────────────────────────────

    /// List all containers with their current state.
    pub async fn list(&self, all: bool) -> Result<()> {
        let containers = self.client.list(all).await?;

        if containers.is_empty() {
            println!("No containers found.");
            return Ok(());
        }

        println!("{:<30} {:<12} {}", "NAME", "STATE", "IMAGE");
        println!("{}", "─".repeat(72));
        for c in &containers {
            println!("{:<30} {:<12} {}", c.name, c.state, c.image);
        }
        Ok(())
    }
}
