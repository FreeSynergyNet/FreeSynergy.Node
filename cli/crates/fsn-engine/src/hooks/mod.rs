// Post-deploy hook system.
//
// After a service is healthy, its hook runs to do what Ansible's
// deploy-setup.yml used to do:
//   - create data directories with correct ownership
//   - render config file templates (e.g. kanidm.toml, tuwunel.toml)
//   - initialize admin accounts (podman exec commands)
//   - register OAuth2 clients in Kanidm
//   - emit helpful setup hints
//
// Each hook is idempotent: it writes a `.initialized` marker after the
// first run and skips the initialisation block on subsequent deploys.

pub mod common;
pub mod cryptpad;
pub mod forgejo;
pub mod kanidm;
pub mod stalwart;
pub mod tuwunel;
pub mod vikunja;

use std::path::{Path, PathBuf};

use anyhow::Result;
use fsn_core::{
    config::{ProjectConfig, VaultConfig},
    state::desired::{DesiredState, ModuleInstance},
};

/// Everything a hook needs to do its work.
pub struct HookContext<'a> {
    /// The module instance that was just deployed.
    pub instance: &'a ModuleInstance,

    /// All modules in the project (needed by Kanidm to register OAuth2 clients).
    pub desired: &'a DesiredState,

    pub project: &'a ProjectConfig,
    pub vault:   &'a VaultConfig,

    /// Root of all project data directories: `{fsn_root}/projects/{project_slug}/data/`
    pub data_root: PathBuf,

    /// FSN repo root (for finding module templates).
    pub fsn_root: &'a Path,
}

impl<'a> HookContext<'a> {
    /// Data directory for this specific instance.
    /// Mirrors `module_vars.config_dir` in Ansible.
    pub fn instance_data_dir(&self) -> PathBuf {
        self.data_root.join(&self.instance.name)
    }

    /// Path to the module's templates/ directory.
    pub fn templates_dir(&self) -> PathBuf {
        // class_key = "auth/kanidm"  →  modules/auth/kanidm/templates/
        let parts: Vec<&str> = self.instance.class_key.splitn(3, '/').collect();
        let mut p = self.fsn_root.join("modules");
        for part in parts { p = p.join(part); }
        p.join("templates")
    }

    /// Idempotency marker file.
    pub fn initialized_marker(&self) -> PathBuf {
        self.instance_data_dir().join(".initialized")
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized_marker().exists()
    }

    pub fn mark_initialized(&self) -> Result<()> {
        let m = self.initialized_marker();
        if let Some(p) = m.parent() { std::fs::create_dir_all(p)?; }
        std::fs::write(&m, "")?;
        Ok(())
    }
}

/// Dispatch post-deploy hook for the given instance (if one is registered).
pub async fn run_hook(ctx: &HookContext<'_>) -> Result<()> {
    match ctx.instance.class_key.as_str() {
        "auth/kanidm"                                    => kanidm::run(ctx).await,
        "git/forgejo"                                    => forgejo::run(ctx).await,
        "mail/stalwart"                                  => stalwart::run(ctx).await,
        "collab/cryptpad"                                => cryptpad::run(ctx).await,
        "chat/tuwunel"                                   => tuwunel::run(ctx).await,
        "tasks/vikunja"                                  => vikunja::run(ctx).await,
        "observability/openobserver"                     => openobserver_stub(ctx),
        _                                                => common::ensure_data_dir(ctx),
    }
}

/// Generic fallback: just make sure the data dir exists.
fn openobserver_stub(ctx: &HookContext<'_>) -> Result<()> {
    common::ensure_data_dir(ctx)?;
    tracing::info!(
        "{}: ready. Login at https://{}  (admin credentials in vault)",
        ctx.instance.name, ctx.instance.service_domain
    );
    Ok(())
}
