// Zentinel KDL config generation.
//
// Generates the FSN-managed section of the Zentinel proxy config.
// Strategy: the entire upstreams{} + routes{} block is regenerated on every
// deploy and written between FSN-MANAGED markers — manually-edited parts
// (listeners, server settings) above/below the markers are never touched.
//
// Real Zentinel KDL format (docs.zentinelproxy.io/v/25.12/configuration/):
//
//   upstreams {
//     upstream "name" {
//       targets {
//         target {
//           address "host:port"
//         }
//       }
//     }
//   }
//   routes {
//     route "name" {
//       matches {
//         host "domain.example.com"
//       }
//       upstream "name"
//     }
//   }
//
// Zentinel is Pingora-based — NOT Caddy. The KDL syntax is its own.

use fsn_node_core::{
    config::service::ServiceType,
    state::desired::{DesiredState, ServiceInstance},
};

const MARKER_START: &str = "# === FSN-MANAGED-START ===";
const MARKER_END:   &str = "# === FSN-MANAGED-END ===";

// ── Public API ────────────────────────────────────────────────────────────────

/// Replace (or insert) the FSN-managed section in an existing Zentinel config.
/// Everything outside the markers is preserved verbatim.
pub fn upsert_managed_section(config: &str, desired: &DesiredState) -> String {
    let managed = generate_managed_section(desired);
    match (config.find(MARKER_START), config.find(MARKER_END)) {
        (Some(s), Some(e)) => {
            let end_of_block = e + MARKER_END.len();
            format!("{}{}{}", &config[..s], managed, &config[end_of_block..])
        }
        _ => format!("{}\n{}\n", config.trim_end(), managed),
    }
}

/// Generate the complete Zentinel config file from scratch (initial install).
/// Writes a minimal server + listeners block plus the FSN-managed section.
pub fn generate_full_config(desired: &DesiredState) -> String {
    let managed = generate_managed_section(desired);
    format!(
        "# Zentinel proxy configuration\n\
         # Lines outside the FSN-MANAGED block can be edited freely.\n\
         \n\
         listeners {{\n\
         \x20   listener \"http\" {{\n\
         \x20       address \"0.0.0.0:80\"\n\
         \x20   }}\n\
         \x20   listener \"https\" {{\n\
         \x20       address \"0.0.0.0:443\"\n\
         \x20   }}\n\
         }}\n\
         \n\
         {managed}\n"
    )
}

/// Remove a single service from the managed section.
/// Pass the filtered `DesiredState` (without the removed service) to regenerate.
pub fn upsert_without(config: &str, desired: &DesiredState) -> String {
    upsert_managed_section(config, desired)
}

// ── Core generation ───────────────────────────────────────────────────────────

/// Build the complete FSN-managed KDL block (upstreams + routes).
fn generate_managed_section(desired: &DesiredState) -> String {
    let instances = collect_proxy_instances(desired);

    let mut upstreams = String::new();
    let mut routes    = String::new();

    for inst in &instances {
        upstreams.push_str(&upstream_block(inst));
        routes.push_str(&route_blocks(inst));
    }

    format!(
        "{MARKER_START}\nupstreams {{\n{upstreams}}}\nroutes {{\n{routes}}}\n{MARKER_END}\n"
    )
}

/// Generate one `upstream "name" { targets { target { address "…:port" } } }` block.
fn upstream_block(inst: &ServiceInstance) -> String {
    let name = &inst.name;
    let port = inst.class.meta.port;
    // Containers reach each other by container name on the internal network.
    format!(
        "    upstream \"{name}\" {{\n\
         \x20       targets {{\n\
         \x20           target {{\n\
         \x20               address \"{name}:{port}\"\n\
         \x20           }}\n\
         \x20       }}\n\
         \x20   }}\n"
    )
}

/// Generate `route` blocks for all domains (primary + aliases) of a service.
fn route_blocks(inst: &ServiceInstance) -> String {
    let name = &inst.name;
    let mut all_domains = vec![inst.service_domain.clone()];
    all_domains.extend(inst.alias_domains.clone());

    let mut out = String::new();
    for domain in &all_domains {
        // Use domain as route name with dots replaced (KDL names must be unique).
        let route_name = domain.replace('.', "-");
        out.push_str(&format!(
            "    route \"{route_name}\" {{\n\
             \x20       matches {{\n\
             \x20           host \"{domain}\"\n\
             \x20       }}\n\
             \x20       upstream \"{name}\"\n\
             \x20   }}\n"
        ));
    }
    out
}

// ── Helper ────────────────────────────────────────────────────────────────────

/// Collect all instances (incl. sub-services) that need an HTTP proxy route.
/// Excludes internal services (Database, Cache) and the proxy itself.
fn collect_proxy_instances(desired: &DesiredState) -> Vec<ServiceInstance> {
    let mut out = Vec::new();
    for inst in &desired.services {
        push_proxy_instance(inst, &mut out);
    }
    out
}

fn push_proxy_instance(inst: &ServiceInstance, out: &mut Vec<ServiceInstance>) {
    // Include services that are user-facing and not the proxy itself.
    if !inst.class.meta.is_internal_only() && !inst.class.meta.has_type(&ServiceType::Proxy) {
        out.push(inst.clone());
    }
    for sub in &inst.sub_services {
        push_proxy_instance(sub, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use indexmap::IndexMap;
    use fsn_node_core::{
        config::service::{
            Constraints, ContainerDef, ServiceClass, ServiceContract,
            ServiceLoad, ServiceMeta, ServiceSetup, ServiceType,
        },
        state::desired::{DesiredState, ServiceInstance},
    };

    fn make_class(name: &str, port: u16, service_types: Vec<ServiceType>) -> ServiceClass {
        ServiceClass {
            meta: ServiceMeta {
                name: name.to_string(),
                alias: vec![],
                service_types,
                author: None,
                version: "1.0".to_string(),
                tags: vec![],
                description: None,
                website: None,
                repository: None,
                port,
                constraints: Constraints::default(),
                federation: None,
                health_path: None,
                health_port: None,
                health_scheme: None,
                capabilities: vec![],
                roles: Default::default(),
                ui: Default::default(),
            },
            vars: IndexMap::default(),
            load: ServiceLoad::default(),
            container: ContainerDef {
                name: name.to_string(),
                image: format!("docker.io/{name}"),
                image_tag: "latest".to_string(),
                networks: vec![],
                volumes: vec![],
                published_ports: vec![],
                healthcheck: None,
                user: None,
                read_only: false,
                tmpfs: vec![],
                security_opt: vec![],
                ulimits: IndexMap::default(),
            },
            environment: IndexMap::default(),
            setup: ServiceSetup::default(),
            contract: ServiceContract::default(),
            manifest: None,
        }
    }

    fn make_instance(name: &str, domain: &str, service_types: Vec<ServiceType>, port: u16) -> ServiceInstance {
        ServiceInstance {
            name: name.to_string(),
            class_key: format!("test/{name}"),
            class: make_class(name, port, service_types.clone()),
            service_types,
            resolved_env: HashMap::new(),
            service_domain: domain.to_string(),
            alias_domains: vec![],
            sub_services: vec![],
            version: "1.0".to_string(),
            resolved_volumes: vec![],
            capabilities: vec![],
        }
    }

    fn desired(services: Vec<ServiceInstance>) -> DesiredState {
        DesiredState { project_name: "myproject".to_string(), domain: "example.com".to_string(), services }
    }

    #[test]
    fn full_config_contains_markers() {
        let config = generate_full_config(&desired(vec![
            make_instance("forgejo", "git.example.com", vec![ServiceType::Git], 3000),
        ]));
        assert!(config.contains(MARKER_START));
        assert!(config.contains(MARKER_END));
    }

    #[test]
    fn full_config_contains_upstream_and_route_for_git() {
        let config = generate_full_config(&desired(vec![
            make_instance("forgejo", "git.example.com", vec![ServiceType::Git], 3000),
        ]));
        assert!(config.contains(r#"upstream "forgejo""#));
        assert!(config.contains("forgejo:3000"));
        assert!(config.contains("git.example.com"));
    }

    #[test]
    fn proxy_service_excluded_from_routes() {
        let config = generate_full_config(&desired(vec![
            make_instance("zentinel", "example.com", vec![ServiceType::Proxy], 443),
        ]));
        assert!(!config.contains(r#"upstream "zentinel""#));
    }

    #[test]
    fn internal_service_excluded_from_routes() {
        let config = generate_full_config(&desired(vec![
            make_instance("postgres", "postgres.internal", vec![ServiceType::Database], 5432),
        ]));
        assert!(!config.contains(r#"upstream "postgres""#));
    }

    #[test]
    fn domain_dots_replaced_in_route_name() {
        let config = generate_full_config(&desired(vec![
            make_instance("forgejo", "git.example.com", vec![ServiceType::Git], 3000),
        ]));
        assert!(config.contains(r#"route "git-example-com""#));
    }

    #[test]
    fn alias_domains_generate_extra_routes() {
        let mut inst = make_instance("forgejo", "git.example.com", vec![ServiceType::Git], 3000);
        inst.alias_domains = vec!["git2.example.com".to_string()];
        let config = generate_full_config(&desired(vec![inst]));
        assert!(config.contains("git.example.com"));
        assert!(config.contains("git2.example.com"));
        assert!(config.contains(r#"route "git2-example-com""#));
    }

    #[test]
    fn upsert_replaces_managed_section_between_markers() {
        let existing = format!(
            "# hand-written config\n{MARKER_START}\nold content\n{MARKER_END}\n# after marker\n"
        );
        let result = upsert_managed_section(&existing, &desired(vec![
            make_instance("forgejo", "git.example.com", vec![ServiceType::Git], 3000),
        ]));
        assert!(result.starts_with("# hand-written config\n"));
        assert!(result.ends_with("# after marker\n"));
        assert!(!result.contains("old content"));
        assert!(result.contains(r#"upstream "forgejo""#));
    }

    #[test]
    fn upsert_appends_when_no_markers_present() {
        let existing = "# existing config\n";
        let result = upsert_managed_section(existing, &desired(vec![
            make_instance("forgejo", "git.example.com", vec![ServiceType::Git], 3000),
        ]));
        assert!(result.starts_with("# existing config"));
        assert!(result.contains(MARKER_START));
        assert!(result.contains(MARKER_END));
    }
}
