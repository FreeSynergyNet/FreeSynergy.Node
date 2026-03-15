//! SSH session management via russh.

use std::sync::Arc;

use anyhow::{Context, Result, bail};
use russh::client::{self, Handle};
use russh::keys::{key, load_secret_key};
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::RemoteHost;

/// Output of a remote command execution.
#[derive(Debug, Clone)]
pub struct ExecOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: u32,
}

impl ExecOutput {
    /// Returns Ok if exit code is 0, else Err with stderr message.
    pub fn into_result(self) -> Result<String> {
        if self.exit_code == 0 {
            Ok(self.stdout)
        } else {
            bail!(
                "remote command failed (exit {}): {}",
                self.exit_code,
                self.stderr.trim()
            )
        }
    }
}

// ── russh client handler (minimal: accept any host key) ──────────────────────

struct ClientHandler;

#[async_trait::async_trait]
impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        // TODO: verify against known_hosts for production hardening
        Ok(true)
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Returns the first existing standard SSH key path.
fn default_key_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    for name in ["id_ed25519", "id_rsa", "id_ecdsa"] {
        let path = format!("{home}/.ssh/{name}");
        if std::path::Path::new(&path).exists() {
            return path;
        }
    }
    format!("{home}/.ssh/id_ed25519")
}

// ── SshSession ────────────────────────────────────────────────────────────────

/// An active SSH session to a remote host.
pub struct SshSession {
    handle: Arc<Mutex<Handle<ClientHandler>>>,
}

impl SshSession {
    /// Open an SSH connection to `host`.
    /// Authentication order: key file (if set) → SSH agent.
    pub async fn connect(host: &RemoteHost) -> Result<Self> {
        info!("SSH connect → {}@{}:{}", host.ssh_user, host.address, host.ssh_port);

        let config = Arc::new(client::Config::default());
        let addr   = format!("{}:{}", host.address, host.ssh_port);

        let mut handle = client::connect(config, addr, ClientHandler)
            .await
            .with_context(|| format!("TCP connect to {}", host.address))?;

        // ── Key authentication ──────────────────────────────────────────────
        let key_path = host.ssh_key_path.clone().unwrap_or_else(default_key_path);
        let key = load_secret_key(&key_path, None)
            .with_context(|| format!("loading private key {key_path}"))?;

        let authenticated = handle
            .authenticate_publickey(&host.ssh_user, Arc::new(key))
            .await
            .context("public-key authentication")?;

        if !authenticated {
            bail!("SSH public-key authentication rejected for {}", host.ssh_user);
        }

        debug!("SSH authenticated as {}", host.ssh_user);
        Ok(Self {
            handle: Arc::new(Mutex::new(handle)),
        })
    }

    /// Execute a shell command on the remote host.
    pub async fn exec(&self, cmd: &str) -> Result<ExecOutput> {
        debug!("SSH exec: {cmd}");
        let guard = self.handle.lock().await;

        let mut channel = guard
            .channel_open_session()
            .await
            .context("open SSH channel")?;

        channel.exec(true, cmd.as_bytes()).await.context("exec")?;

        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit_code = 0u32;

        loop {
            let Some(msg) = channel.wait().await else { break };
            match msg {
                russh::ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                russh::ChannelMsg::ExtendedData { data, .. } => stderr.extend_from_slice(&data),
                russh::ChannelMsg::ExitStatus { exit_status } => exit_code = exit_status,
                russh::ChannelMsg::Eof => break,
                _ => {}
            }
        }
        channel.close().await.ok();

        Ok(ExecOutput {
            stdout:    String::from_utf8_lossy(&stdout).into_owned(),
            stderr:    String::from_utf8_lossy(&stderr).into_owned(),
            exit_code,
        })
    }

    /// Write `content` to `remote_path` on the remote host (via `cat >` shell redirect).
    pub async fn write_file(&self, remote_path: &str, content: &[u8]) -> Result<()> {
        debug!("SSH write_file → {remote_path}");
        let guard = self.handle.lock().await;

        // Create parent directory
        let parent = std::path::Path::new(remote_path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or(".");
        let mkdir_cmd = format!("mkdir -p {}", shell_escape(parent));

        let mut mkdir_ch = guard.channel_open_session().await.context("open channel mkdir")?;
        mkdir_ch.exec(true, mkdir_cmd.as_bytes()).await?;
        drain_channel(&mut mkdir_ch).await?;
        mkdir_ch.close().await.ok();

        // Write via stdin of `cat >`
        let write_cmd = format!("cat > {}", shell_escape(remote_path));
        let mut write_ch = guard.channel_open_session().await.context("open channel write")?;
        write_ch.exec(true, write_cmd.as_bytes()).await?;

        // Send content as AsyncRead
        let mut cursor = std::io::Cursor::new(content);
        write_ch.data(&mut cursor).await?;
        write_ch.eof().await?;
        drain_channel(&mut write_ch).await?;
        write_ch.close().await.ok();

        Ok(())
    }

    /// Close the SSH connection gracefully.
    pub async fn close(self) -> Result<()> {
        let guard = self.handle.lock().await;
        guard.disconnect(russh::Disconnect::ByApplication, "", "en").await.ok();
        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Single-quote escape a shell argument.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Drain a channel until EOF or close.
async fn drain_channel(ch: &mut russh::Channel<client::Msg>) -> Result<()> {
    loop {
        let Some(msg) = ch.wait().await else { break };
        match msg {
            russh::ChannelMsg::Eof | russh::ChannelMsg::Close => break,
            _ => {}
        }
    }
    Ok(())
}
