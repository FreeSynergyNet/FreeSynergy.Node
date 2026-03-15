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
    state::desired::{DesiredState, ServiceInstance},
};

/// Everything a hook needs to do its work.
pub struct HookContext<'a> {
    /// The module instance that was just deployed.
    pub instance: &'a ServiceInstance,

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

// ── Hook registry ──────────────────────────────────────────────────────────────

/// Async hook function pointer type.
type HookFn = for<'a> fn(&'a HookContext<'a>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>>;

/// Static mapping from `class_key` to hook implementation.
///
/// Entries are checked in order; the first matching key wins.
/// If no key matches, `common::ensure_data_dir` is used as the default.
static HOOK_REGISTRY: &[(&str, HookFn)] = &[
    ("auth/kanidm",              |ctx| Box::pin(kanidm::run(ctx))),
    ("git/forgejo",              |ctx| Box::pin(forgejo::run(ctx))),
    ("mail/stalwart",            |ctx| Box::pin(stalwart::run(ctx))),
    ("collab/cryptpad",          |ctx| Box::pin(cryptpad::run(ctx))),
    ("chat/tuwunel",             |ctx| Box::pin(tuwunel::run(ctx))),
    ("tasks/vikunja",            |ctx| Box::pin(vikunja::run(ctx))),
    ("observability/openobserver", |ctx| Box::pin(openobserver_hook(ctx))),
];

/// Dispatch post-deploy hook for the given instance (if one is registered).
pub async fn run_hook(ctx: &HookContext<'_>) -> Result<()> {
    let hook = HOOK_REGISTRY
        .iter()
        .find(|(key, _)| *key == ctx.instance.class_key.as_str())
        .map(|(_, f)| *f);

    match hook {
        Some(f) => f(ctx).await,
        None    => common::ensure_data_dir(ctx),
    }
}

/// Hook for openobserver: ensure data dir + log login hint.
async fn openobserver_hook(ctx: &HookContext<'_>) -> Result<()> {
    common::ensure_data_dir(ctx)?;
    tracing::info!(
        "{}: ready. Login at https://{}  (admin credentials in vault)",
        ctx.instance.name, ctx.instance.service_domain
    );
    Ok(())
}
