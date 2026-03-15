// Template rendering via fsn-template (Tera engine).
//
// FSN-specific context (`FsnTemplateContext`) holds all the domain fields
// and converts to `fsn_template::TemplateContext` for rendering.
// `ProxyServiceSpec` is an FSN-specific data type used by proxy module templates.

use std::collections::HashMap;

use anyhow::Result;
use serde::Serialize;

use fsn_template::{TemplateContext as LibCtx, TemplateEngine};

/// FSN-level alias for callers within this crate.
pub type TemplateContext<'a> = FsnTemplateContext<'a>;
use fsn_node_core::config::{RouteSpec, VaultConfig};

// ── ProxyServiceSpec ──────────────────────────────────────────────────────────

/// One service that needs proxy routing.
///
/// Derived from the service's `ServiceContract` at resolve time.
/// Proxy module templates iterate over `proxy_services` to generate routing config.
#[derive(Debug, Clone, Serialize)]
pub struct ProxyServiceSpec {
    /// Instance name (e.g. `"kanidm"`, `"outline"`).
    pub name: String,
    /// Full service domain (e.g. `"kanidm.example.com"`).
    pub domain: String,
    /// Resolved container name (e.g. `"kanidm"`).
    pub container: String,
    /// Primary internal port.
    pub port: u16,
    /// Routes declared in the service's `[contract]` block.
    pub routes: Vec<RouteSpec>,
    /// Whether the upstream (container) uses TLS internally.
    pub upstream_tls: bool,
    /// Proxy health-check path (from `contract.health_path` or `module.health_path`).
    pub health_path: Option<String>,
}

// ── FsnTemplateContext ────────────────────────────────────────────────────────

/// FSN-specific template rendering context.
///
/// Holds all domain-level fields and converts to [`TemplateContext`] for rendering.
pub struct FsnTemplateContext<'a> {
    /// Project short name (e.g. `"fsn-net"`).
    pub project_name: &'a str,
    /// Primary domain (e.g. `"example.com"`).
    pub project_domain: &'a str,
    /// Instance name (e.g. `"zentinel"`).
    pub instance_name: &'a str,
    /// Fully qualified service domain.
    pub service_domain: &'a str,
    /// Parent instance name (same as `instance_name` for top-level services).
    pub parent_instance_name: &'a str,
    /// Filesystem root of the project (parent of the `data/` directory).
    pub project_root: &'a str,
    /// Vault configuration (for injecting `vault_*` secrets).
    pub vault: &'a VaultConfig,
    /// Cross-service and project-level variables from `VarProvider` exports.
    pub cross_vars: HashMap<String, String>,
    /// Pre-computed `[vars]` block from the module TOML.
    pub module_vars: HashMap<String, String>,
    /// Expanded plugin vars (dns_provider, acme_email, acme_ca_url, …).
    pub plugin_vars: HashMap<String, String>,
    /// Services that need proxy routing — available as `{{ proxy_services }}`.
    pub proxy_services: Vec<ProxyServiceSpec>,
}

impl<'a> FsnTemplateContext<'a> {
    /// Convert to a [`TemplateContext`] ready for rendering.
    pub fn to_fsn(&self) -> Result<LibCtx> {
        let mut ctx = LibCtx::new();

        ctx.set_str("project_name",         self.project_name);
        ctx.set_str("project_domain",        self.project_domain);
        ctx.set_str("instance_name",         self.instance_name);
        ctx.set_str("service_domain",        self.service_domain);
        ctx.set_str("parent_instance_name",  self.parent_instance_name);
        ctx.set_str("project_root",          self.project_root);

        // module_vars as a nested object so `{{ module_vars.config_dir }}` works.
        ctx.set("module_vars", &self.module_vars)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // Cross-service vars: inject lowercase so `{{ mail_host }}` etc. work.
        let lower_cross: HashMap<String, String> = self.cross_vars
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v.clone()))
            .collect();
        ctx.merge_str_map(&lower_cross);

        // Plugin vars (dns_provider, acme_email, …) as flat top-level vars.
        ctx.merge_str_map(&self.plugin_vars);

        // proxy_services as structured list.
        ctx.set("proxy_services", &self.proxy_services)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        // Vault secrets: inject all exposed keys.
        for key in self.vault.keys() {
            if let Some(value) = self.vault.expose(key) {
                ctx.set_str(key, value);
            }
        }

        Ok(ctx)
    }
}

// ── Render helpers ────────────────────────────────────────────────────────────

/// Render a single Jinja2/Tera template string with the given FSN context.
pub fn render(template: &str, ctx: &FsnTemplateContext) -> Result<String> {
    let engine = TemplateEngine::new();
    let lib_ctx = ctx.to_fsn()?;
    engine.render_str(template, &lib_ctx)
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Render a multi-line template file (e.g. `container.quadlet.j2`).
pub fn render_file(template_content: &str, ctx: &FsnTemplateContext) -> Result<String> {
    render(template_content, ctx)
}

