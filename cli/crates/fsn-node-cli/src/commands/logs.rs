use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub async fn run(service: &str, follow: bool) -> Result<()> {
    let mut cmd = Command::new("journalctl");
    cmd.args(["--user", "-u", &format!("{}.service", service), "--no-pager"]);
    if follow {
        cmd.arg("-f");
    }

    // Stream output line by line
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to run journalctl for '{}'", service))?;

    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout).lines();

    while let Some(line) = reader.next_line().await? {
        println!("{}", line);
    }

    child.wait().await?;
    Ok(())
}
