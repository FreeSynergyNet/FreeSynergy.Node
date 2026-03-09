// Jinja2-compatible template engine – wraps minijinja.
//
// The existing .j2 templates in playbooks/templates/ work unchanged.
// Variable names match the Ansible template context (instance_name,
// project_root, service_domain, vault_*, ...).

use anyhow::Result;
use minijinja::Environment;
use std::collections::HashMap;

use fsn_core::config::{RouteSpec, VaultConfig};

/// Template rendering context – mirrors the Ansible variable namespace.
pub struct TemplateContext<'a> {
    pub project_name: &'a str,
    pub project_domain: &'a str,
    pub instance_name: &'a str,
    pub service_domain: &'a str,
    pub parent_instance_name: &'a str,
    /// Filesystem root of the project (parent of the data/ directory).
    /// Matches `{{ project_root }}` in module TOML templates.
    pub project_root: &'a str,
    pub vault: &'a VaultConfig,
    /// Cross-service and project-level variables from VarProvider exports.
    /// Injected before vault secrets so secrets can always override.
    pub cross_vars: HashMap<String, String>,
    /// Pre-computed [vars] block from the module TOML.
    /// Exposed as `{{ module_vars.config_dir }}` etc. in templates.
    pub module_vars: HashMap<String, String>,
    /// Expanded plugin vars (dns_provider, acme_email, acme_ca_url, …).
    /// Injected as flat top-level vars so templates can use `{{ dns_provider }}` etc.
    /// Empty for non-proxy modules.
    pub plugin_vars: HashMap<String, String>,
    /// Services that need proxy routing — built from ServiceContracts.
    /// Available as `{{ proxy_services }}` list in proxy module templates.
    /// Each entry is an object with: name, domain, container, port, upstream_tls,
    /// health_path (optional), routes (list of {id, path, strip}).
    /// Empty for non-proxy modules.
    pub proxy_services: Vec<ProxyServiceSpec>,
}

/// Describes one service that needs proxy routing.
/// Derived from the service's `ServiceContract` at resolve time.
/// Proxy module templates iterate over `proxy_services` to generate routing config.
#[derive(Debug, Clone)]
pub struct ProxyServiceSpec {
    /// Instance name (e.g. "kanidm", "outline").
    pub name: String,
    /// Full service domain (e.g. "kanidm.example.com").
    pub domain: String,
    /// Resolved container name (e.g. "kanidm").
    pub container: String,
    /// Primary internal port.
    pub port: u16,
    /// Routes declared in the service's [contract] block.
    pub routes: Vec<RouteSpec>,
    /// Whether the upstream (container) uses TLS internally.
    pub upstream_tls: bool,
    /// Proxy health-check path (from contract.health_path or module.health_path).
    pub health_path: Option<String>,
}

/// Render a single Jinja2 template string with the given context.
pub fn render(template: &str, ctx: &TemplateContext) -> Result<String> {
    let env = Environment::new();

    // Build variable map – includes core vars plus vault secrets
    let mut vars: HashMap<String, minijinja::Value> = [
        ("project_name",          ctx.project_name),
        ("project_domain",        ctx.project_domain),
        ("instance_name",         ctx.instance_name),
        ("service_domain",        ctx.service_domain),
        ("parent_instance_name",  ctx.parent_instance_name),
        ("project_root",          ctx.project_root),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), minijinja::Value::from(v)))
    .collect();

    // Inject module_vars as a dict so {{ module_vars.config_dir }} etc. work.
    let mv_obj = minijinja::Value::from_iter(
        ctx.module_vars.iter().map(|(k, v)| (k.as_str(), minijinja::Value::from(v.as_str())))
    );
    vars.insert("module_vars".into(), mv_obj);

    // Inject cross-service vars (e.g. MAIL_HOST, IAM_URL) so services can reference each other.
    // Keys are lowercased for Jinja2 compatibility: {{ mail_host }}, {{ iam_url }}, etc.
    for (k, v) in &ctx.cross_vars {
        vars.insert(k.to_lowercase(), minijinja::Value::from(v.as_str()));
    }

    // Inject plugin vars (dns_provider, acme_email, acme_ca_url, …) as flat top-level vars.
    // Applied after cross_vars so plugin-specific values don't clash with project vars.
    for (k, v) in &ctx.plugin_vars {
        vars.insert(k.clone(), minijinja::Value::from(v.as_str()));
    }

    // Inject proxy_services as a structured list for proxy module templates.
    // Usage: {% for svc in proxy_services %} ... {{ svc.domain }} ... {% endfor %}
    let svc_list: Vec<minijinja::Value> = ctx.proxy_services.iter().map(|svc| {
        let routes: Vec<minijinja::Value> = svc.routes.iter().map(|r| {
            minijinja::Value::from_iter([
                ("id",          minijinja::Value::from(r.id.as_str())),
                ("path",        minijinja::Value::from(r.path.as_str())),
                ("strip",       minijinja::Value::from(r.strip)),
                ("description", minijinja::Value::from(
                    r.description.as_deref().unwrap_or("")
                )),
            ])
        }).collect();

        let mut obj: Vec<(&str, minijinja::Value)> = vec![
            ("name",         minijinja::Value::from(svc.name.as_str())),
            ("domain",       minijinja::Value::from(svc.domain.as_str())),
            ("container",    minijinja::Value::from(svc.container.as_str())),
            ("port",         minijinja::Value::from(u64::from(svc.port))),
            ("upstream_tls", minijinja::Value::from(svc.upstream_tls)),
            ("routes",       minijinja::Value::from(routes)),
        ];
        if let Some(hp) = &svc.health_path {
            obj.push(("health_path", minijinja::Value::from(hp.as_str())));
        }
        minijinja::Value::from_iter(obj)
    }).collect();
    vars.insert("proxy_services".into(), minijinja::Value::from(svc_list));

    // Inject vault secrets (vault_* keys) into the template context.
    // Vault values are exposed only at render time, never stored as plain strings.
    for key in ctx.vault.keys() {
        if let Some(exposed) = ctx.vault.expose(key) {
            vars.insert(key.to_string(), minijinja::Value::from(exposed));
        }
    }

    let tmpl = env.template_from_str(template)?;
    Ok(tmpl.render(vars)?)
}

/// Render a multi-line template file (e.g. container.quadlet.j2).
pub fn render_file(template_content: &str, ctx: &TemplateContext) -> Result<String> {
    render(template_content, ctx)
}
