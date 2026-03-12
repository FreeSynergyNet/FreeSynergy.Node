// State resolution – build DesiredState from config files.
//
// Algorithm:
//   1. Pre-compute cross-service vars (MAIL_HOST, IAM_URL, …) from project entries.
//   2. For each module entry in project.yml load.services:
//      a. Look up the module class in ServiceRegistry
//      b. Resolve sub-modules recursively
//      c. Expand Jinja2 vars with expand_template() (includes cross-service vars)
//      d. Build ServiceInstance
//   3. Enforce that instance names are unique (duplicate = error)
//   4. Return DesiredState

use std::collections::HashMap;

use anyhow::{bail, Context, Result};

use fsn_core::{
    config::{HostConfig, ServiceRegistry, ProjectConfig, VaultConfig},
    config::service::ServiceType,
    resource::ProjectResource,
    state::desired::{DesiredState, ServiceInstance},
};

// collect_proxy_services is pub for use in hooks and deploy pipelines.

use crate::template::TemplateContext;

/// Build the desired state from the three config layers.
///
/// `data_root` – when `Some`, volumes in module TOMLs are rendered with a
///   resolved `{{ project_root }}` and `{{ module_vars.* }}` context.
///   Pass `None` in non-deploy contexts (init wizard, web API, sync diff).
pub fn resolve_desired(
    project: &ProjectConfig,
    host: &HostConfig,
    registry: &ServiceRegistry,
    vault: &VaultConfig,
    data_root: Option<&std::path::Path>,
) -> Result<DesiredState> {
    // Pre-compute cross-service vars from all service entries.
    // Done once before resolution so every service can reference sibling services.
    let cross_vars = collect_cross_service_vars(project);

    // Compute project_root: parent of data_root (e.g. "projects/fsn-net/")
    // so {{ project_root }}/data/{{ instance_name }} expands correctly.
    let project_root_buf;
    let project_root: &str = match data_root {
        Some(dr) => {
            project_root_buf = dr.parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            &project_root_buf
        }
        None => "",
    };

    let mut instances = Vec::new();
    let mut seen_names = HashMap::new();

    for (instance_name, module_ref) in &project.load.services {
        // Uniqueness check (per RULES.md: duplicate instance name = abort)
        if let Some(existing) = seen_names.insert(instance_name.clone(), module_ref.service_class.clone()) {
            bail!(
                "Duplicate service name '{}' in project '{}' (already defined as {})",
                instance_name,
                project.project.meta.name,
                existing
            );
        }

        let instance = resolve_instance(
            instance_name,
            &module_ref.service_class,
            &module_ref.env,
            project,
            host,
            registry,
            vault,
            None, // no parent
            &cross_vars,
            project_root,
        )
        .with_context(|| format!("Resolving module '{}'", instance_name))?;

        instances.push(instance);
    }

    Ok(DesiredState {
        project_name: project.project.meta.name.clone(),
        domain: project.project.domain.clone(),
        services: instances,
    })
}

/// Pre-compute cross-service variables from the project load entries.
///
/// Derived from instance names + project domain before ServiceClass loading,
/// so no chicken-and-egg problem. Each service can reference sibling services
/// via `{{ mail_host }}`, `{{ iam_url }}`, etc. in its Jinja2 environment block.
///
/// Uses `ServiceType::from_class_prefix()` + `ServiceType::exported_contract()`
/// as the single source of truth for the prefix mapping — no local match block.
pub fn collect_cross_service_vars(project: &ProjectConfig) -> HashMap<String, String> {
    let mut vars = HashMap::new();

    // Project-level vars
    vars.insert("PROJECT_NAME".into(),   project.project.meta.name.clone());
    vars.insert("PROJECT_DOMAIN".into(), project.project.domain.clone());
    if let Some(email) = project.contact_email() {
        vars.insert("PROJECT_EMAIL".into(), email.to_string());
    }

    // Cross-service vars (MAIL_HOST, IAM_URL, GIT_DOMAIN, etc.)
    for (instance_name, entry) in &project.load.services {
        let class_prefix = entry.service_class.split('/').next().unwrap_or("");
        let Some(stype)    = ServiceType::from_class_prefix(class_prefix) else { continue };
        let Some(contract) = stype.exported_contract()                    else { continue };

        let subdomain = entry.subdomain.as_deref().unwrap_or(instance_name.as_str());
        let domain    = format!("{}.{}", subdomain, project.project.domain);
        let port      = entry.port.unwrap_or(0);

        vars.extend(contract.resolve(instance_name, &domain, port));
    }

    vars
}

/// Resolve a single module instance (and its sub-modules recursively).
fn resolve_instance(
    name: &str,
    class_key: &str,
    instance_env: &indexmap::IndexMap<String, String>,
    project: &ProjectConfig,
    host: &HostConfig,
    registry: &ServiceRegistry,
    vault: &VaultConfig,
    parent_name: Option<&str>,
    cross_vars: &HashMap<String, String>,
    project_root: &str,
) -> Result<ServiceInstance> {
    let class = registry
        .get(class_key)
        .with_context(|| format!("Module class '{}' not found in registry", class_key))?
        .clone();

    let service_domain = format!("{}.{}", name, project.project.domain);
    let alias_domains: Vec<String> = class
        .meta
        .alias
        .iter()
        .map(|a| format!("{}.{}", a, project.project.domain))
        .collect();

    // Pre-compute [vars] block: render each var template with just the basic vars
    // (no module_vars self-reference). This gives us e.g. config_dir = "/projects/fsn-net/data/zentinel".
    let module_vars = precompute_module_vars(&class.vars, project_root, name, &project.project.meta.name, &project.project.domain);

    // Collect plugin vars for proxy modules (dns_provider, acme_email, acme_ca_url, …).
    // For all other module types this is an empty map.
    let plugin_vars = if class_key.starts_with("proxy/") {
        collect_plugin_vars(host, registry)
    } else {
        HashMap::new()
    };

    // Collect proxy service specs for proxy modules.
    // Proxy templates iterate over `proxy_services` to generate per-service routing config.
    let proxy_services = if class_key.starts_with("proxy/") {
        collect_proxy_services(project, registry, project_root)
    } else {
        Vec::new()
    };

    // Build template context for Jinja2 expansion (includes cross-service vars)
    let ctx = TemplateContext {
        project_name: &project.project.meta.name,
        project_domain: &project.project.domain,
        instance_name: name,
        service_domain: &service_domain,
        parent_instance_name: parent_name.unwrap_or(name),
        project_root,
        vault,
        cross_vars: cross_vars.clone(),
        module_vars,
        plugin_vars,
        proxy_services,
    };

    // Expand environment variables (module defaults + instance overrides)
    let mut resolved_env = resolve_env(&class.environment, &ctx)?;
    // Instance-level env overrides take precedence over module defaults
    for (k, v) in instance_env {
        resolved_env.insert(k.clone(), v.clone());
    }

    // Expand volume mount strings ({{ module_vars.config_dir }}/data:/data:Z → real path)
    let resolved_volumes = resolve_volumes(&class.container.volumes, &ctx)?;

    // Resolve sub-modules recursively (same cross_vars for the whole project)
    let mut sub_services = Vec::new();
    for (sub_name_tpl, sub_ref) in &class.load.sub_services {
        let sub_name = format!("{}-{}", name, sub_name_tpl);
        let sub = resolve_instance(
            &sub_name,
            &sub_ref.service_class,
            &indexmap::IndexMap::new(),
            project,
            host,
            registry,
            vault,
            Some(name),
            cross_vars,
            project_root,
        )
        .with_context(|| format!("Resolving sub-module '{}'", sub_name))?;
        sub_services.push(sub);
    }

    // Merge capability set: type defaults + plugin-declared extras.
    let mut capabilities: Vec<fsn_core::config::Capability> = class.meta.service_types.iter()
        .flat_map(|t| t.capabilities())
        .collect();
    for cap in &class.meta.capabilities {
        if !capabilities.contains(cap) {
            capabilities.push(cap.clone());
        }
    }

    Ok(ServiceInstance {
        name: name.to_string(),
        class_key: class_key.to_string(),
        service_types: class.meta.service_types.clone(),
        version: class.meta.version.clone(),
        capabilities,
        class,
        resolved_env,
        resolved_volumes,
        service_domain,
        alias_domains,
        sub_services,
    })
}

/// Expand all Jinja2 strings in the environment block.
fn resolve_env(
    raw_env: &indexmap::IndexMap<String, String>,
    ctx: &TemplateContext,
) -> Result<HashMap<String, String>> {
    let mut out = HashMap::new();
    for (key, template) in raw_env {
        let value = crate::template::render(template, ctx)
            .with_context(|| format!("Expanding env var '{}'", key))?;
        out.insert(key.clone(), value);
    }
    Ok(out)
}

/// Expand Jinja2 templates in volume mount strings.
fn resolve_volumes(raw_volumes: &[String], ctx: &TemplateContext) -> Result<Vec<String>> {
    raw_volumes
        .iter()
        .map(|tpl| {
            crate::template::render(tpl, ctx)
                .with_context(|| format!("Expanding volume '{}'", tpl))
        })
        .collect()
}

/// Collect proxy service specs from all services that have declared routes.
///
/// Called when resolving a proxy module instance — provides `proxy_services`
/// in the Jinja2 template context so proxy templates can iterate:
///   `{% for svc in proxy_services %}{{ svc.domain }} { ... }{% endfor %}`
///
/// Services without `[contract.routes]` (e.g. databases, caches, the proxy itself)
/// are excluded automatically.
pub fn collect_proxy_services(
    project: &ProjectConfig,
    registry: &ServiceRegistry,
    _project_root: &str,
) -> Vec<crate::template::ProxyServiceSpec> {
    let mut specs = Vec::new();

    for (instance_name, entry) in &project.load.services {
        let Some(class) = registry.get(&entry.service_class) else { continue };

        // Skip services with no declared routes — nothing to proxy.
        if class.contract.routes.is_empty() { continue }

        // Skip services that are purely internal infrastructure (Database, Cache).
        if class.meta.is_internal_only() { continue }

        let subdomain = entry.subdomain.as_deref().unwrap_or(instance_name.as_str());
        let domain = format!("{}.{}", subdomain, project.project.domain);

        // Resolve container name: replace {{ instance_name }} placeholder.
        let container = class.container.name
            .replace("{{ instance_name }}", instance_name)
            .replace("{{ parent_instance_name }}", instance_name);

        let health_path = class.contract.health_path.clone()
            .or_else(|| class.meta.health_path.clone());

        specs.push(crate::template::ProxyServiceSpec {
            name: instance_name.clone(),
            domain,
            container,
            port: class.meta.port,
            routes: class.contract.routes.clone(),
            upstream_tls: class.contract.upstream_tls,
            health_path,
        });
    }

    specs
}

/// Collect expanded plugin vars for a proxy module instance.
///
/// Reads the first proxy entry in host.proxy, loads the referenced DNS and ACME
/// plugins from the registry, and merges their vars into a flat map.
/// The ACME email is injected from ProxyPlugins.acme_email → host.acme.email → "".
fn collect_plugin_vars(host: &HostConfig, registry: &ServiceRegistry) -> HashMap<String, String> {
    let mut vars: HashMap<String, String> = HashMap::new();

    // Use the first proxy entry (per RULES.md: per_host = 1, so there is exactly one)
    let Some((_, proxy)) = host.proxy.iter().next() else { return vars };
    let plugins = &proxy.load.plugins;

    // Determine service_type from proxy.service_class (e.g. "proxy/zentinel" → "proxy")
    let service_type = proxy.service_class.split('/').next().unwrap_or("proxy");

    // Load DNS plugin vars
    if let Some(dns_plugin) = registry.get_plugin(service_type, "dns", &plugins.dns) {
        vars.extend(dns_plugin.vars.clone());
    }

    // Load ACME plugin vars
    if let Some(acme_plugin) = registry.get_plugin(service_type, "acme", &plugins.acme) {
        vars.extend(acme_plugin.vars.clone());
    }

    // ACME email: proxy override → host-level default → empty string
    let acme_email = plugins.acme_email.as_deref()
        .or_else(|| host.acme.as_ref().map(|a| a.email.as_str()))
        .unwrap_or_default();
    vars.insert("acme_email".into(), acme_email.to_string());

    vars
}

/// Pre-compute the [vars] block from a module class.
///
/// Each var value is itself a Jinja2 template (may reference `project_root`,
/// `instance_name`, `project_name`, `project_domain`). We render them with a
/// minimal context (no module_vars self-reference) to get concrete paths/strings.
fn precompute_module_vars(
    vars: &indexmap::IndexMap<String, toml::Value>,
    project_root: &str,
    instance_name: &str,
    project_name: &str,
    project_domain: &str,
) -> HashMap<String, String> {
    use minijinja::Environment;
    let mut out = HashMap::new();
    let env = Environment::new();

    let base_vars: HashMap<String, minijinja::Value> = [
        ("project_root",   project_root),
        ("instance_name",  instance_name),
        ("project_name",   project_name),
        ("project_domain", project_domain),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), minijinja::Value::from(v)))
    .collect();

    for (key, val) in vars {
        let template_str = match val {
            toml::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let rendered = env.template_from_str(&template_str)
            .and_then(|t| t.render(&base_vars))
            .unwrap_or_else(|_| template_str.clone());
        out.insert(key.clone(), rendered);
    }
    out
}
